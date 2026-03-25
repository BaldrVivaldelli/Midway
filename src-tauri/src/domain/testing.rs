use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::http::{ResolvedPair, ResponseEnvelope};

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum AssertionSource {
    Status,
    Header,
    BodyText,
    JsonPointer,
    FinalUrl,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum AssertionOperator {
    Equals,
    Contains,
    NotContains,
    Exists,
    NotExists,
    GreaterOrEqual,
    LessOrEqual,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResponseAssertion {
    pub id: String,
    pub name: String,
    pub enabled: bool,
    pub source: AssertionSource,
    pub operator: AssertionOperator,
    pub selector: Option<String>,
    pub expected: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AssertionResult {
    pub id: String,
    pub name: String,
    pub passed: bool,
    pub source: AssertionSource,
    pub operator: AssertionOperator,
    pub selector: Option<String>,
    pub expected: String,
    pub actual: Option<String>,
    pub message: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AssertionReport {
    pub total: u64,
    pub passed: u64,
    pub failed: u64,
    pub results: Vec<AssertionResult>,
}

pub fn evaluate_response_assertions(
    response: &ResponseEnvelope,
    assertions: &[ResponseAssertion],
) -> AssertionReport {
    let mut report = AssertionReport::default();

    for assertion in assertions.iter().filter(|assertion| assertion.enabled) {
        let (passed, actual, message) = evaluate_single_assertion(response, assertion);

        if passed {
            report.passed += 1;
        } else {
            report.failed += 1;
        }

        report.results.push(AssertionResult {
            id: assertion.id.clone(),
            name: assertion.name.clone(),
            passed,
            source: assertion.source.clone(),
            operator: assertion.operator.clone(),
            selector: assertion.selector.clone(),
            expected: assertion.expected.clone(),
            actual,
            message,
        });
    }

    report.total = report.passed + report.failed;
    report
}

fn evaluate_single_assertion(
    response: &ResponseEnvelope,
    assertion: &ResponseAssertion,
) -> (bool, Option<String>, String) {
    let actual_value = extract_actual_value(response, &assertion.source, assertion.selector.as_deref());

    match assertion.operator {
        AssertionOperator::Exists => match actual_value {
            Ok(Some(actual)) => (
                true,
                Some(actual),
                "La aserción existe y devolvió un valor.".to_string(),
            ),
            Ok(None) => (
                false,
                None,
                "Se esperaba un valor, pero no existe.".to_string(),
            ),
            Err(message) => (false, None, message),
        },
        AssertionOperator::NotExists => match actual_value {
            Ok(Some(actual)) => (
                false,
                Some(actual),
                "Se esperaba ausencia, pero hubo un valor disponible.".to_string(),
            ),
            Ok(None) => (
                true,
                None,
                "La aserción confirmó que el valor no existe.".to_string(),
            ),
            Err(message) => (false, None, message),
        },
        AssertionOperator::Equals => compare_text(actual_value, &assertion.expected, |actual, expected| actual == expected, "igual"),
        AssertionOperator::Contains => compare_text(actual_value, &assertion.expected, |actual, expected| actual.contains(expected), "contener"),
        AssertionOperator::NotContains => compare_text(actual_value, &assertion.expected, |actual, expected| !actual.contains(expected), "no contener"),
        AssertionOperator::GreaterOrEqual => compare_numbers(actual_value, &assertion.expected, |actual, expected| actual >= expected, "ser mayor o igual"),
        AssertionOperator::LessOrEqual => compare_numbers(actual_value, &assertion.expected, |actual, expected| actual <= expected, "ser menor o igual"),
    }
}

fn compare_text(
    actual_value: Result<Option<String>, String>,
    expected: &str,
    predicate: impl Fn(&str, &str) -> bool,
    verb: &str,
) -> (bool, Option<String>, String) {
    match actual_value {
        Ok(Some(actual)) => {
            let passed = predicate(&actual, expected);
            let message = if passed {
                format!("La aserción pasó: el valor cumple con {verb} \"{expected}\".")
            } else {
                format!("La aserción falló: se esperaba {verb} \"{expected}\".")
            };
            (passed, Some(actual), message)
        }
        Ok(None) => (
            false,
            None,
            format!("No hubo valor para comparar y se esperaba {verb} \"{expected}\"."),
        ),
        Err(message) => (false, None, message),
    }
}

fn compare_numbers(
    actual_value: Result<Option<String>, String>,
    expected: &str,
    predicate: impl Fn(f64, f64) -> bool,
    verb: &str,
) -> (bool, Option<String>, String) {
    let expected_number = match expected.trim().parse::<f64>() {
        Ok(value) => value,
        Err(_) => {
            return (
                false,
                None,
                format!("El valor esperado \"{expected}\" no es un número válido."),
            )
        }
    };

    match actual_value {
        Ok(Some(actual)) => match actual.trim().parse::<f64>() {
            Ok(actual_number) => {
                let passed = predicate(actual_number, expected_number);
                let message = if passed {
                    format!(
                        "La aserción pasó: {actual_number} cumple con {verb} {expected_number}."
                    )
                } else {
                    format!(
                        "La aserción falló: {actual_number} no cumple con {verb} {expected_number}."
                    )
                };
                (passed, Some(actual), message)
            }
            Err(_) => (
                false,
                Some(actual),
                "El valor actual no es un número válido para comparar.".to_string(),
            ),
        },
        Ok(None) => (
            false,
            None,
            "No hubo valor actual para comparar numéricamente.".to_string(),
        ),
        Err(message) => (false, None, message),
    }
}

fn extract_actual_value(
    response: &ResponseEnvelope,
    source: &AssertionSource,
    selector: Option<&str>,
) -> Result<Option<String>, String> {
    match source {
        AssertionSource::Status => Ok(Some(response.status.to_string())),
        AssertionSource::BodyText => Ok(Some(response.body_text.clone())),
        AssertionSource::FinalUrl => Ok(Some(response.final_url.clone())),
        AssertionSource::Header => {
            let header_name = selector
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .ok_or_else(|| "La aserción sobre header necesita un selector con el nombre del header.".to_string())?;

            Ok(find_header(&response.headers, header_name).map(|pair| pair.value.clone()))
        }
        AssertionSource::JsonPointer => {
            let pointer = selector
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .ok_or_else(|| "La aserción sobre JSON necesita un selector con un JSON Pointer.".to_string())?;

            let json = serde_json::from_str::<Value>(&response.body_text)
                .map_err(|error| format!("No se pudo parsear el body como JSON: {error}"))?;

            Ok(json.pointer(pointer).map(json_value_to_string))
        }
    }
}

fn find_header<'a>(headers: &'a [ResolvedPair], name: &str) -> Option<&'a ResolvedPair> {
    headers
        .iter()
        .find(|header| header.key.eq_ignore_ascii_case(name))
}

fn json_value_to_string(value: &Value) -> String {
    match value {
        Value::Null => "null".to_string(),
        Value::Bool(boolean) => boolean.to_string(),
        Value::Number(number) => number.to_string(),
        Value::String(text) => text.clone(),
        Value::Array(_) | Value::Object(_) => serde_json::to_string_pretty(value)
            .unwrap_or_else(|_| value.to_string()),
    }
}
