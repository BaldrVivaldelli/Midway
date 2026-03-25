use std::collections::BTreeSet;

use super::http::{AuthConfig, FormDataRow, KeyValueRow, RequestDraft};

pub fn collect_secret_aliases(
    draft: &RequestDraft,
    environment_rows: &[KeyValueRow],
) -> BTreeSet<String> {
    let mut aliases = BTreeSet::new();

    collect_in_text(&draft.url, &mut aliases);
    collect_rows(&draft.query, &mut aliases);
    collect_rows(&draft.headers, &mut aliases);
    collect_in_text(&draft.body.value, &mut aliases);
    collect_form_data_rows(&draft.body.form_data, &mut aliases);

    match &draft.auth {
        AuthConfig::None => {}
        AuthConfig::Bearer { token } => collect_in_text(token, &mut aliases),
        AuthConfig::Basic { username, password } => {
            collect_in_text(username, &mut aliases);
            collect_in_text(password, &mut aliases);
        }
        AuthConfig::ApiKey { key, value, .. } => {
            collect_in_text(key, &mut aliases);
            collect_in_text(value, &mut aliases);
        }
    }

    collect_rows(environment_rows, &mut aliases);

    aliases
}

fn collect_rows(rows: &[KeyValueRow], aliases: &mut BTreeSet<String>) {
    for row in rows {
        collect_in_text(&row.key, aliases);
        collect_in_text(&row.value, aliases);
    }
}

fn collect_in_text(text: &str, aliases: &mut BTreeSet<String>) {
    let mut remainder = text;

    while let Some(start) = remainder.find("{{") {
        let after_start = &remainder[start + 2..];

        let Some(end) = after_start.find("}}") else {
            break;
        };

        let token = after_start[..end].trim();

        if let Some(alias) = token.strip_prefix("secret:") {
            let alias = alias.trim();

            if !alias.is_empty() {
                aliases.insert(alias.to_string());
            }
        }

        remainder = &after_start[end + 2..];
    }
}

fn collect_form_data_rows(rows: &[FormDataRow], aliases: &mut BTreeSet<String>) {
    for row in rows {
        collect_in_text(&row.key, aliases);
        collect_in_text(&row.value, aliases);
        if let Some(file_name) = &row.file_name {
            collect_in_text(file_name, aliases);
        }
    }
}
