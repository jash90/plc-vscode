pub(crate) fn collect_execution_output(text: &str) -> Vec<String> {
    collect_initialized_string_variables(text)
}

fn collect_initialized_string_variables(text: &str) -> Vec<String> {
    let mut output = Vec::new();
    let mut in_var_block = false;

    for raw_line in text.lines() {
        let line = raw_line.trim();
        let upper = line.to_ascii_uppercase();

        if upper == "VAR" || upper.starts_with("VAR_") {
            in_var_block = true;
            continue;
        }

        if upper == "END_VAR" {
            in_var_block = false;
            continue;
        }

        if !in_var_block || line.is_empty() || line.starts_with("//") {
            continue;
        }

        if let Some((name, value)) = parse_string_initialization(line) {
            output.push(format!("{name} = {value}"));
        }
    }

    output
}

fn parse_string_initialization(line: &str) -> Option<(String, String)> {
    let (name, rest) = line.split_once(':')?;
    let name = name.trim();
    if name.is_empty() || !is_identifier(name) {
        return None;
    }

    let (type_name, initializer) = rest.split_once(":=")?;
    let type_name = type_name.trim().to_ascii_uppercase();
    if type_name != "STRING" && !type_name.starts_with("STRING[") {
        return None;
    }

    let initializer = initializer.trim().trim_end_matches(';').trim();
    let quote = initializer.chars().next()?;
    if quote != '\'' && quote != '"' {
        return None;
    }

    let after_quote = &initializer[quote.len_utf8()..];
    let end_quote = after_quote.find(quote)?;
    Some((name.to_owned(), after_quote[..end_quote].to_owned()))
}

fn is_identifier(candidate: &str) -> bool {
    let mut chars = candidate.chars();
    let Some(first) = chars.next() else {
        return false;
    };

    (first == '_' || first.is_ascii_alphabetic())
        && chars.all(|character| character == '_' || character.is_ascii_alphanumeric())
}
