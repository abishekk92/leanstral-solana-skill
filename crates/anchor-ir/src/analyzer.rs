use crate::ir::{
    AccountFieldIr, AccountsStructIr, AnalysisIr, ConstraintIr, InstructionIr,
    PreconditionIr, PreconditionKind, PropertyCandidateIr, TestSignalIr, TransferIr,
};
use anchor_syn::{AccountField, AccountsStruct, ConstraintGroup, Program, Ty};
use quote::ToTokens;
use regex::Regex;
use serde::Deserialize;
use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};
use syn::{Attribute, File, Item};

pub fn analyze_project(
    idl_path: Option<&Path>,
    input: Option<&Path>,
    tests: &[PathBuf],
    output_dir: Option<&Path>,
) -> Result<String, String> {
    let source = if let Some(input) = input {
        Some(fs::read_to_string(input).map_err(|e| e.to_string())?)
    } else {
        None
    };
    let parsed = if let Some(source) = &source {
        Some(syn::parse_file(source).map_err(|e| e.to_string())?)
    } else {
        None
    };

    let mut instructions = Vec::new();
    let mut accounts = Vec::new();

    if let Some(idl_path) = idl_path {
        let idl_source = fs::read_to_string(idl_path).map_err(|e| e.to_string())?;
        let idl: AnchorIdl = serde_json::from_str(&idl_source).map_err(|e| e.to_string())?;
        instructions = parse_idl_instructions(&idl);
        accounts = parse_idl_accounts(&idl);
    }

    if let Some(parsed) = &parsed {
        merge_instructions(&mut instructions, parse_program(parsed)?);
        merge_accounts(&mut accounts, parse_accounts(parsed)?);
    }

    let test_signals = parse_tests(tests)?;
    let property_candidates = derive_property_candidates(&instructions, &accounts, &test_signals);

    let ir = AnalysisIr {
        source_file: input
            .map(|path| path.display().to_string())
            .unwrap_or_default(),
        idl_file: idl_path.map(|path| path.display().to_string()),
        test_files: tests.iter().map(|p| p.display().to_string()).collect(),
        instructions,
        accounts,
        test_signals,
        property_candidates,
    };

    let json = serde_json::to_string_pretty(&ir).map_err(|e| e.to_string())?;
    if let Some(output_dir) = output_dir {
        fs::create_dir_all(output_dir).map_err(|e| e.to_string())?;
        fs::write(output_dir.join("analysis.json"), &json).map_err(|e| e.to_string())?;

        Ok(format!(
            "Wrote analysis to {}",
            output_dir.join("analysis.json").display()
        ))
    } else {
        Ok(json)
    }
}

#[derive(Debug, Deserialize)]
struct AnchorIdl {
    #[serde(default)]
    instructions: Vec<IdlInstruction>,
    #[serde(default)]
    accounts: Vec<IdlAccountDef>,
}

#[derive(Debug, Deserialize)]
struct IdlInstruction {
    name: String,
    #[serde(default)]
    args: Vec<IdlArg>,
    #[serde(default)]
    accounts: Vec<IdlInstructionAccount>,
}

#[derive(Debug, Deserialize)]
struct IdlArg {
    name: String,
    #[serde(rename = "type")]
    ty: serde_json::Value,
}

#[derive(Debug, Deserialize)]
struct IdlInstructionAccount {
    name: String,
    #[serde(default)]
    signer: bool,
    #[serde(default)]
    _writable: bool,
    #[serde(default)]
    pda: Option<IdlPda>,
}

#[derive(Debug, Deserialize)]
struct IdlPda {
    #[serde(default)]
    seeds: Vec<IdlSeed>,
}

#[derive(Debug, Deserialize)]
struct IdlSeed {
    #[serde(default)]
    _kind: String,
    #[serde(default)]
    value: Option<serde_json::Value>,
    #[serde(default)]
    path: Option<String>,
}

#[derive(Debug, Deserialize)]
struct IdlAccountDef {
    name: String,
    #[serde(default)]
    _discriminator: Vec<u8>,
}

fn parse_idl_instructions(idl: &AnchorIdl) -> Vec<InstructionIr> {
    idl.instructions
        .iter()
        .map(|ix| {
            let args: Vec<String> = ix
                .args
                .iter()
                .map(|arg| format!("{}: {}", arg.name, idl_type_label(&arg.ty)))
                .collect();

            let preconditions = extract_type_preconditions(&args);

            InstructionIr {
                name: ix.name.clone(),
                context_type: infer_context_type(&ix.name),
                args,
                pda_seeds: ix
                    .accounts
                    .iter()
                    .flat_map(|acct| acct.pda.iter())
                    .flat_map(|pda| pda.seeds.iter())
                    .filter_map(seed_label)
                    .collect(),
                closes_accounts: Vec::new(),
                auth_signals: ix
                    .accounts
                    .iter()
                    .filter(|acct| acct.signer)
                    .map(|acct| format!("idl signer: {}", acct.name))
                    .collect(),
                transfers: Vec::new(),
                preconditions,
                evidence_sources: vec!["idl".into()],
            }
        })
        .collect()
}

fn seed_label(seed: &IdlSeed) -> Option<String> {
    if let Some(path) = &seed.path {
        return Some(path.clone());
    }
    match &seed.value {
        Some(serde_json::Value::String(s)) => Some(s.clone()),
        Some(serde_json::Value::Array(bytes)) => {
            let values: Vec<u8> = bytes
                .iter()
                .filter_map(|value| value.as_u64().and_then(|n| u8::try_from(n).ok()))
                .collect();
            String::from_utf8(values).ok().or_else(|| Some("const_seed".into()))
        }
        Some(other) => Some(other.to_string()),
        None => None,
    }
}

fn idl_type_label(value: &serde_json::Value) -> String {
    match value {
        serde_json::Value::String(s) => s.clone(),
        other => other.to_string(),
    }
}

fn parse_idl_accounts(idl: &AnchorIdl) -> Vec<AccountsStructIr> {
    idl.accounts
        .iter()
        .map(|account| AccountsStructIr {
            name: account.name.clone(),
            fields: Vec::new(),
            evidence_sources: vec!["idl".into()],
        })
        .collect()
}

fn infer_context_type(name: &str) -> String {
    let mut chars = name.chars();
    match chars.next() {
        Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
        None => String::new(),
    }
}

fn merge_instructions(existing: &mut Vec<InstructionIr>, incoming: Vec<InstructionIr>) {
    for instruction in incoming {
        if let Some(current) = existing.iter_mut().find(|ix| ix.name == instruction.name) {
            merge_string_vec(&mut current.args, instruction.args);
            merge_string_vec(&mut current.pda_seeds, instruction.pda_seeds);
            merge_string_vec(&mut current.closes_accounts, instruction.closes_accounts);
            merge_string_vec(&mut current.auth_signals, instruction.auth_signals);
            current.transfers.extend(instruction.transfers);
            current.preconditions.extend(instruction.preconditions);
            merge_string_vec(&mut current.evidence_sources, instruction.evidence_sources);
            if current.context_type.is_empty() {
                current.context_type = instruction.context_type;
            }
        } else {
            existing.push(instruction);
        }
    }
}

fn merge_accounts(existing: &mut Vec<AccountsStructIr>, incoming: Vec<AccountsStructIr>) {
    for account in incoming {
        if let Some(current) = existing.iter_mut().find(|a| a.name == account.name) {
            current.fields = account.fields;
            merge_string_vec(&mut current.evidence_sources, account.evidence_sources);
        } else {
            existing.push(account);
        }
    }
}

fn merge_string_vec(current: &mut Vec<String>, incoming: Vec<String>) {
    let mut set: BTreeSet<String> = current.iter().cloned().collect();
    set.extend(incoming);
    *current = set.into_iter().collect();
}

fn parse_program(file: &File) -> Result<Vec<InstructionIr>, String> {
    let mut instructions = Vec::new();

    for item in &file.items {
        if let Item::Mod(item_mod) = item {
            if has_attr(&item_mod.attrs, "program") {
                let program: Program = syn::parse2(item_mod.to_token_stream())
                    .map_err(|e| format!("failed to parse #[program] module: {e}"))?;

                for ix in program.ixs {
                    let body = ix.raw_method.block.to_token_stream().to_string();
                    let args: Vec<String> = ix
                        .args
                        .iter()
                        .map(|arg| format!("{}: {}", arg.name, arg.raw_arg.ty.to_token_stream()))
                        .collect();

                    let preconditions = extract_type_preconditions(&args);

                    instructions.push(InstructionIr {
                        name: ix.ident.to_string(),
                        context_type: ix.anchor_ident.to_string(),
                        args,
                        pda_seeds: extract_seed_literals(&body),
                        closes_accounts: extract_close_targets(&body),
                        auth_signals: extract_auth_signals(&body),
                        transfers: extract_transfers(&body),
                        preconditions,
                        evidence_sources: vec!["rust".into()],
                    });
                }
            }
        }
    }

    Ok(instructions)
}

fn parse_accounts(file: &File) -> Result<Vec<AccountsStructIr>, String> {
    let mut accounts = Vec::new();

    for item in &file.items {
        if let Item::Struct(item_struct) = item {
            if has_derive_accounts(&item_struct.attrs) {
                let parsed: AccountsStruct = syn::parse2(item_struct.to_token_stream())
                    .map_err(|e| format!("failed to parse Accounts struct {}: {e}", item_struct.ident))?;
                accounts.push(AccountsStructIr {
                    name: parsed.ident.to_string(),
                    fields: parsed.fields.iter().map(account_field_to_ir).collect(),
                    evidence_sources: vec!["rust".into()],
                });
            }
        }
    }

    Ok(accounts)
}

fn parse_tests(test_files: &[PathBuf]) -> Result<Vec<TestSignalIr>, String> {
    let re = Regex::new(r#"(it|test)\s*\(\s*["'`](.*?)["'`]"#).map_err(|e| e.to_string())?;
    let mut signals = Vec::new();

    for test_file in test_files {
        let source = fs::read_to_string(test_file).map_err(|e| e.to_string())?;
        for capture in re.captures_iter(&source) {
            let name = capture.get(2).map(|m| m.as_str()).unwrap_or_default().to_string();
            signals.push(TestSignalIr {
                file: test_file.display().to_string(),
                inferred_properties: infer_properties_from_text(&name),
                name,
            });
        }
    }

    Ok(signals)
}

fn derive_property_candidates(
    instructions: &[InstructionIr],
    accounts: &[AccountsStructIr],
    test_signals: &[TestSignalIr],
) -> Vec<PropertyCandidateIr> {
    let mut candidates = Vec::new();
    let test_props: BTreeSet<String> = test_signals
        .iter()
        .flat_map(|signal| signal.inferred_properties.iter().cloned())
        .collect();

    for instruction in instructions {
        let account_struct = accounts
            .iter()
            .find(|account| account.name == instruction.context_type);

        let has_auth_constraint = account_struct.map(|account| {
            account.fields.iter().any(|field| {
                field.is_signer
                    || field
                        .constraints
                        .iter()
                        .any(|constraint| matches!(constraint.kind.as_str(), "has_one" | "owner" | "address"))
            })
        });
        let has_close_constraint = account_struct.map(|account| {
            account.fields.iter().any(|field| {
                field.constraints
                    .iter()
                    .any(|constraint| constraint.kind == "close")
            })
        });

        if has_auth_constraint.unwrap_or(false)
            || !instruction.auth_signals.is_empty()
            || test_props.contains("access_control")
        {
            // Extract authorization preconditions
            let auth_preconditions = if let Some(account) = account_struct {
                extract_auth_preconditions(&account.fields, &instruction.auth_signals)
            } else {
                Vec::new()
            };

            candidates.push(PropertyCandidateIr {
                id: format!("{}_access_control", instruction.name),
                category: "access_control".into(),
                title: format!("{}: access control", instruction.name),
                confidence: "high".into(),
                relevant_instructions: vec![instruction.name.clone()],
                evidence: instruction.auth_signals.clone(),
                preconditions: auth_preconditions,
                prompt_hint: format!(
                    "Model only the authorization condition for {}. Prove success implies the caller matches the required authority relation.",
                    instruction.name
                ),
            });
        }

        if !instruction.transfers.is_empty() || test_props.contains("cpi_correctness") {
            // Type preconditions are relevant for CPI parameter validation
            let cpi_preconditions = instruction.preconditions.clone();

            candidates.push(PropertyCandidateIr {
                id: format!("{}_cpi_correctness", instruction.name),
                category: "cpi_correctness".into(),
                title: format!("{}: CPI parameter correctness", instruction.name),
                confidence: if instruction.transfers.len() > 1 {
                    "high".into()
                } else {
                    "medium".into()
                },
                relevant_instructions: vec![instruction.name.clone()],
                evidence: instruction
                    .transfers
                    .iter()
                    .map(|transfer| {
                        format!(
                            "transfer {:?} -> {:?} authority {:?} amount {:?}",
                            transfer.from, transfer.to, transfer.authority, transfer.amount_expr
                        )
                    })
                    .collect(),
                preconditions: cpi_preconditions,
                prompt_hint: format!(
                    "Model the CPI parameters constructed by {}. Prove the transfer CPIs have valid parameters: correct program ID, distinct from/to accounts, bounded amounts, and correct authorities.",
                    instruction.name
                ),
            });
        }

        if has_close_constraint.unwrap_or(false)
            || !instruction.closes_accounts.is_empty()
            || test_props.contains("state_machine")
        {
            // Extract state preconditions
            let state_preconditions = if let Some(account) = account_struct {
                extract_state_preconditions(&account.fields)
            } else {
                Vec::new()
            };

            candidates.push(PropertyCandidateIr {
                id: format!("{}_state_machine", instruction.name),
                category: "state_machine".into(),
                title: format!("{}: close / one-shot safety", instruction.name),
                confidence: "medium".into(),
                relevant_instructions: vec![instruction.name.clone()],
                evidence: instruction.closes_accounts.clone(),
                preconditions: state_preconditions,
                prompt_hint: format!(
                    "Model only the lifecycle flag for the state account. Prove {} moves it to a terminal closed state.",
                    instruction.name
                ),
            });
        }
    }

    if instructions
        .iter()
        .flat_map(|ix| ix.args.iter())
        .any(|arg| arg.contains("u8") || arg.contains("u16") || arg.contains("u32") || arg.contains("u64") || arg.contains("u128"))
        || test_props.contains("arithmetic_safety")
    {
        // Collect all type bound preconditions from all instructions
        let arithmetic_preconditions: Vec<PreconditionIr> = instructions
            .iter()
            .flat_map(|ix| ix.preconditions.iter())
            .filter(|p| matches!(p.kind, PreconditionKind::TypeBound { .. }))
            .cloned()
            .collect();

        candidates.push(PropertyCandidateIr {
            id: "program_arithmetic_safety".into(),
            category: "arithmetic_safety".into(),
            title: "Program arithmetic safety".into(),
            confidence: "medium".into(),
            relevant_instructions: instructions.iter().map(|ix| ix.name.clone()).collect(),
            evidence: instructions
                .iter()
                .flat_map(|ix| ix.args.iter().cloned())
                .filter(|arg| arg.contains('u'))
                .collect(),
            preconditions: arithmetic_preconditions,
            prompt_hint:
                "Model only the numeric parameters and arithmetic preconditions that matter. Avoid token/account semantics unless required."
                    .into(),
        });
    }

    candidates
}

fn account_field_to_ir(field: &AccountField) -> AccountFieldIr {
    match field {
        AccountField::Field(field) => AccountFieldIr {
            name: field.ident.to_string(),
            ty: ty_to_string(&field.ty),
            is_signer: field.constraints.is_signer() || matches!(field.ty, Ty::Signer),
            is_mutable: field.constraints.is_mutable(),
            constraints: constraints_to_ir(&field.constraints),
        },
        AccountField::CompositeField(field) => AccountFieldIr {
            name: field.ident.to_string(),
            ty: field.symbol.clone(),
            is_signer: field.constraints.is_signer(),
            is_mutable: field.constraints.is_mutable(),
            constraints: constraints_to_ir(&field.constraints),
        },
    }
}

fn constraints_to_ir(group: &ConstraintGroup) -> Vec<ConstraintIr> {
    let mut constraints = Vec::new();

    if group.is_signer() {
        constraints.push(ConstraintIr {
            kind: "signer".into(),
            raw: "signer".into(),
            target: None,
        });
    }
    if group.is_mutable() {
        constraints.push(ConstraintIr {
            kind: "mut".into(),
            raw: "mut".into(),
            target: None,
        });
    }
    for has_one in &group.has_one {
        constraints.push(ConstraintIr {
            kind: "has_one".into(),
            raw: has_one.join_target.to_token_stream().to_string(),
            target: Some(has_one.join_target.to_token_stream().to_string()),
        });
    }
    if let Some(close) = &group.close {
        constraints.push(ConstraintIr {
            kind: "close".into(),
            raw: close.sol_dest.to_token_stream().to_string(),
            target: Some(close.sol_dest.to_token_stream().to_string()),
        });
    }
    if let Some(owner) = &group.owner {
        constraints.push(ConstraintIr {
            kind: "owner".into(),
            raw: owner.owner_address.to_token_stream().to_string(),
            target: Some(owner.owner_address.to_token_stream().to_string()),
        });
    }
    if let Some(address) = &group.address {
        constraints.push(ConstraintIr {
            kind: "address".into(),
            raw: address.address.to_token_stream().to_string(),
            target: Some(address.address.to_token_stream().to_string()),
        });
    }
    if let Some(seeds) = &group.seeds {
        constraints.push(ConstraintIr {
            kind: "seeds".into(),
            raw: seeds.seeds.to_token_stream().to_string(),
            target: None,
        });
    }
    if let Some(token) = &group.token_account {
        constraints.push(ConstraintIr {
            kind: "token_account".into(),
            raw: format!(
                "mint={}, authority={}",
                token.mint.to_token_stream(),
                token.authority.to_token_stream()
            ),
            target: None,
        });
    }
    if let Some(associated) = &group.associated_token {
        constraints.push(ConstraintIr {
            kind: "associated_token".into(),
            raw: format!(
                "mint={}, wallet={}",
                associated.mint.to_token_stream(),
                associated.wallet.to_token_stream()
            ),
            target: None,
        });
    }
    if let Some(init) = &group.init {
        constraints.push(ConstraintIr {
            kind: "init".into(),
            raw: format!("init(if_needed={})", init.if_needed),
            target: None,
        });
    }

    constraints
}

fn ty_to_string(ty: &Ty) -> String {
    match ty {
        Ty::Signer => "Signer".into(),
        Ty::UncheckedAccount => "UncheckedAccount".into(),
        Ty::AccountInfo => "AccountInfo".into(),
        Ty::SystemAccount => "SystemAccount".into(),
        Ty::ProgramData => "ProgramData".into(),
        Ty::Account(account) => account.account_type_path.to_token_stream().to_string(),
        Ty::LazyAccount(account) => account.account_type_path.to_token_stream().to_string(),
        Ty::AccountLoader(account) => account.account_type_path.to_token_stream().to_string(),
        Ty::Program(program) => program.account_type_path.to_token_stream().to_string(),
        Ty::Interface(interface) => interface.account_type_path.to_token_stream().to_string(),
        Ty::InterfaceAccount(account) => account.account_type_path.to_token_stream().to_string(),
        Ty::Sysvar(sysvar) => format!("{sysvar:?}"),
    }
}

fn extract_seed_literals(body: &str) -> Vec<String> {
    Regex::new(r#"b"([^"]+)""#)
        .unwrap()
        .captures_iter(body)
        .filter_map(|capture| capture.get(1).map(|m| m.as_str().to_string()))
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}

fn extract_close_targets(body: &str) -> Vec<String> {
    Regex::new(r#"close\s*=\s*(\w+)"#)
        .unwrap()
        .captures_iter(body)
        .filter_map(|capture| capture.get(1).map(|m| m.as_str().to_string()))
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}

fn extract_auth_signals(body: &str) -> Vec<String> {
    let mut signals = BTreeSet::new();
    for capture in Regex::new(r#"authority\s*:\s*ctx\s*\.\s*accounts\s*\.\s*(\w+)"#)
        .unwrap()
        .captures_iter(body)
    {
        if let Some(name) = capture.get(1) {
            signals.insert(format!("authority: {}", name.as_str()));
        }
    }
    if body.contains("initializer") {
        signals.insert("mentions initializer".into());
    }
    signals.into_iter().collect()
}

fn extract_transfers(body: &str) -> Vec<TransferIr> {
    let mut transfers = Vec::new();
    let from_re = Regex::new(r#"from\s*:\s*ctx\s*\.\s*accounts\s*\.\s*(\w+)"#).unwrap();
    let to_re = Regex::new(r#"to\s*:\s*ctx\s*\.\s*accounts\s*\.\s*(\w+)"#).unwrap();
    let authority_re = Regex::new(r#"authority\s*:\s*ctx\s*\.\s*accounts\s*\.\s*(\w+)"#).unwrap();
    let amount_re = Regex::new(r#"token\s*::\s*transfer\s*\(\s*[^,]+,\s*([A-Za-z0-9_\.]+)\s*\)"#).unwrap();

    let blocks: Vec<&str> = body.split("let cpi_accounts = Transfer").collect();
    for block in blocks.into_iter().skip(1) {
        transfers.push(TransferIr {
            from: from_re
                .captures(block)
                .and_then(|m| m.get(1).map(|v| v.as_str().to_string())),
            to: to_re
                .captures(block)
                .and_then(|m| m.get(1).map(|v| v.as_str().to_string())),
            authority: authority_re
                .captures(block)
                .and_then(|m| m.get(1).map(|v| v.as_str().to_string())),
            amount_expr: amount_re
                .captures(block)
                .and_then(|m| m.get(1).map(|v| v.as_str().to_string())),
            uses_pda_signer: block.contains("new_with_signer"),
        });
    }

    transfers
}

fn infer_properties_from_text(text: &str) -> Vec<String> {
    let lower = text.to_lowercase();
    let mut properties = Vec::new();

    if lower.contains("unauthorized")
        || lower.contains("permission")
        || lower.contains("only")
        || lower.contains("fail")
        || lower.contains("reject")
    {
        properties.push("access_control".into());
    }
    if lower.contains("balance") || lower.contains("transfer") || lower.contains("token") || lower.contains("cpi") {
        properties.push("cpi_correctness".into());
    }
    if lower.contains("close") || lower.contains("reuse") || lower.contains("closed") {
        properties.push("state_machine".into());
    }
    if lower.contains("overflow") || lower.contains("underflow") || lower.contains("u64") {
        properties.push("arithmetic_safety".into());
    }

    properties
}

fn has_attr(attrs: &[Attribute], name: &str) -> bool {
    attrs.iter().any(|attr| attr.path.is_ident(name))
}

fn has_derive_accounts(attrs: &[Attribute]) -> bool {
    attrs.iter().any(|attr| {
        if !attr.path.is_ident("derive") {
            return false;
        }
        attr.to_token_stream().to_string().contains("Accounts")
    })
}

/// Extract type bound preconditions from instruction arguments
fn extract_type_preconditions(args: &[String]) -> Vec<PreconditionIr> {
    let mut preconditions = Vec::new();

    for arg in args {
        // Extract type bounds from arguments like "amount: u64"
        if let Some((var_name, ty)) = arg.split_once(':') {
            let var_name = var_name.trim();
            let ty = ty.trim();

            let (bound_type, upper_bound) = match ty {
                "u8" | "U8" => ("u8", "255"),
                "u16" | "U16" => ("u16", "65535"),
                "u32" | "U32" => ("u32", "4294967295"),
                "u64" | "U64" => ("u64", "18446744073709551615"),
                "u128" | "U128" => ("u128", "340282366920938463463374607431768211455"),
                _ => continue,
            };

            preconditions.push(PreconditionIr {
                kind: PreconditionKind::TypeBound {
                    variable: var_name.to_string(),
                    bound_type: bound_type.to_string(),
                    upper_bound: upper_bound.to_string(),
                },
                description: format!("{} <= {}_MAX", var_name, bound_type.to_uppercase()),
                source: "argument_type".to_string(),
            });
        }
    }

    preconditions
}

/// Extract authorization preconditions from constraints
fn extract_auth_preconditions(
    fields: &[AccountFieldIr],
    auth_signals: &[String],
) -> Vec<PreconditionIr> {
    let mut preconditions = Vec::new();

    // Extract from signer constraints
    for field in fields {
        if field.is_signer {
            for constraint in &field.constraints {
                if constraint.kind == "has_one" {
                    if let Some(target) = &constraint.target {
                        preconditions.push(PreconditionIr {
                            kind: PreconditionKind::Authorization {
                                signer: field.name.clone(),
                                expected: target.clone(),
                            },
                            description: format!("{} must equal {}", field.name, target),
                            source: "has_one_constraint".to_string(),
                        });
                    }
                }
            }
        }
    }

    // Extract from auth signals
    for signal in auth_signals {
        if signal.starts_with("authority: ") {
            let auth = signal.strip_prefix("authority: ").unwrap_or("");
            preconditions.push(PreconditionIr {
                kind: PreconditionKind::Authorization {
                    signer: "signer".to_string(),
                    expected: auth.to_string(),
                },
                description: format!("signer must equal {}", auth),
                source: "auth_signal".to_string(),
            });
        }
    }

    preconditions
}

/// Extract state preconditions from constraints
fn extract_state_preconditions(fields: &[AccountFieldIr]) -> Vec<PreconditionIr> {
    let mut preconditions = Vec::new();

    for field in fields {
        for constraint in &field.constraints {
            match constraint.kind.as_str() {
                "close" => {
                    // Before close, account must not already be closed
                    preconditions.push(PreconditionIr {
                        kind: PreconditionKind::StateConstraint {
                            field: format!("{}.lifecycle", field.name),
                            operator: "!=".to_string(),
                            value: "closed".to_string(),
                        },
                        description: format!("{} must not already be closed", field.name),
                        source: "close_constraint".to_string(),
                    });
                }
                "init" => {
                    // Init requires account doesn't exist yet (implicit)
                }
                _ => {}
            }
        }
    }

    preconditions
}
