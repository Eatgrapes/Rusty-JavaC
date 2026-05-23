use crate::BytecodeError;
use javac_ty::Ty;
use ustr::Ustr;

pub(super) fn unresolved_method(
    method: Ustr,
    arg_types: &[Ty],
    receiver: &str,
    line: Option<u16>,
) -> BytecodeError {
    BytecodeError::at_line(
        format!(
            "cannot find symbol: method {}({}) in {}",
            method,
            display_args(arg_types),
            receiver
        ),
        line,
    )
    .with_needle(method.as_str())
    .with_label("unresolved method call")
    .with_help("check the method name and argument types")
}

pub(super) fn unresolved_variable(name: Ustr, line: Option<u16>) -> BytecodeError {
    BytecodeError::at_line(format!("cannot find symbol: variable {}", name), line)
        .with_needle(name.as_str())
        .with_label("unresolved variable")
        .with_help("declare the variable before using it")
}

pub(super) fn unresolved_field(field: Ustr, receiver: &str, line: Option<u16>) -> BytecodeError {
    BytecodeError::at_line(
        format!("cannot find symbol: variable {} in {}", field, receiver),
        line,
    )
    .with_needle(field.as_str())
    .with_label("unresolved field")
    .with_help("check the field name or add a matching field")
}

pub(super) fn display_internal_name(name: &str) -> String {
    name.replace('/', ".")
}

fn display_args(args: &[Ty]) -> String {
    args.iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>()
        .join(", ")
}
