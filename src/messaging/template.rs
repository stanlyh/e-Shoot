use crate::config::schema::{ComponentDefinition, TemplateDefinition};
use crate::error::Result;

#[derive(Debug, Clone)]
pub struct RenderedTemplate {
    pub name: String,
    pub language: String,
    pub components: Vec<RenderedComponent>,
}

#[derive(Debug, Clone)]
pub struct RenderedComponent {
    pub kind: String,
    pub parameters: Vec<RenderedParameter>,
}

#[derive(Debug, Clone)]
pub struct RenderedParameter {
    pub kind: String,
    pub value: String,
}

pub fn render_for_recipient(
    template: &TemplateDefinition,
    variables: &[String],
) -> Result<RenderedTemplate> {
    let components = template
        .components
        .iter()
        .map(|c| render_component(c, variables))
        .collect::<Result<Vec<_>>>()?;

    Ok(RenderedTemplate {
        name: template.name.clone(),
        language: template.language.clone(),
        components,
    })
}

fn render_component(
    component: &ComponentDefinition,
    variables: &[String],
) -> Result<RenderedComponent> {
    let kind = format!("{:?}", component.kind).to_lowercase();
    let parameters = component
        .parameters
        .iter()
        .map(|p| render_parameter(p, variables))
        .collect::<Result<Vec<_>>>()?;

    Ok(RenderedComponent { kind, parameters })
}

fn render_parameter(
    param: &crate::config::schema::ParameterDefinition,
    variables: &[String],
) -> Result<RenderedParameter> {
    let kind = format!("{:?}", param.kind).to_lowercase();
    let raw_value = param.value.clone().unwrap_or_default();
    let value = substitute_variables(&raw_value, variables)?;
    Ok(RenderedParameter { kind, value })
}

fn substitute_variables(text: &str, variables: &[String]) -> Result<String> {
    let mut result = text.to_string();
    let mut i = 0;
    loop {
        let placeholder = format!("{{{{{}}}}}", i + 1);
        if !result.contains(&placeholder) {
            break;
        }
        let replacement = variables.get(i).map(String::as_str).unwrap_or("");
        result = result.replace(&placeholder, replacement);
        i += 1;
    }
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn substitutes_all_variables() {
        let result = substitute_variables("Hola {{1}}, descuento {{2}}%", &[
            "Eduardo".to_string(),
            "20".to_string(),
        ])
        .unwrap();
        assert_eq!(result, "Hola Eduardo, descuento 20%");
    }

    #[test]
    fn missing_variable_leaves_empty_string() {
        let result = substitute_variables("Hola {{1}} y {{2}}", &["Solo".to_string()]).unwrap();
        assert_eq!(result, "Hola Solo y ");
    }

    #[test]
    fn no_variables_returns_original() {
        let result = substitute_variables("Texto sin variables", &[]).unwrap();
        assert_eq!(result, "Texto sin variables");
    }
}
