use core::time::Duration;
use deno_core::{error::AnyError, op, OpDecl};
use serde::Serialize;
use similar::TextDiff;

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
enum DiffPart {
    Delete(u32),
    Skip(u32),
    Insert(String),
}

#[op]
async fn op_diff_texts(old: String, new: String) -> Result<Vec<DiffPart>, AnyError> {
    let mut _differ = TextDiff::configure();
    let differ = _differ.timeout(Duration::from_secs(1));
    let diff = differ.diff_chars(&old, &new);

    let mut parts = vec![];
    let mut last_op: Option<DiffPart> = None;
    for part in diff.iter_all_changes() {
        let mut new_op: Option<DiffPart> = None;
        match part.tag() {
            similar::ChangeTag::Equal => {
                if let Some(DiffPart::Skip(amount)) = last_op.clone() {
                    last_op = Some(DiffPart::Skip(amount + part.value().len() as u32))
                } else {
                    new_op = Some(DiffPart::Skip(part.value().len() as u32));
                }
            }
            similar::ChangeTag::Delete => {
                if let Some(DiffPart::Delete(amount)) = last_op.clone() {
                    last_op = Some(DiffPart::Delete(amount + part.value().len() as u32))
                } else {
                    new_op = Some(DiffPart::Delete(part.value().len() as u32));
                }
            }
            similar::ChangeTag::Insert => {
                if let Some(DiffPart::Insert(same)) = last_op.clone() {
                    last_op = Some(DiffPart::Insert(same + part.value()))
                } else {
                    new_op = Some(DiffPart::Insert(part.value().to_string()));
                }
            }
        }

        if let Some(new_part) = new_op {
            if let Some(last_part) = last_op.clone() {
                parts.push(last_part);
            }

            last_op = Some(new_part);
        }
    }

    if let Some(op) = last_op {
        match op {
            DiffPart::Skip(_) => {}
            _ => parts.push(op),
        }
    }

    Ok(parts)
}

pub fn get_op_decls() -> Vec<OpDecl> {
    vec![op_diff_texts::decl()]
}
