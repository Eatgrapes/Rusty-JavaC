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
    .with_code("B0103")
    .with_needle(method.as_str())
    .with_label("unresolved method call")
    .with_help("check the method name and argument types")
}

pub(super) fn unresolved_variable(name: Ustr, line: Option<u16>) -> BytecodeError {
    BytecodeError::at_line(format!("cannot find symbol: variable {}", name), line)
        .with_code("B0101")
        .with_needle(name.as_str())
        .with_label("unresolved variable")
        .with_help("declare the variable before using it")
}

pub(super) fn unresolved_field(field: Ustr, receiver: &str, line: Option<u16>) -> BytecodeError {
    BytecodeError::at_line(
        format!("cannot find symbol: variable {} in {}", field, receiver),
        line,
    )
    .with_code("B0102")
    .with_needle(field.as_str())
    .with_label("unresolved field")
    .with_help("check the field name or add a matching field")
}

pub(super) fn invalid_this_method_receiver(method: Ustr, line: Option<u16>) -> BytecodeError {
    BytecodeError::at_line(
        format!("static method {} cannot be called through this", method),
        line,
    )
    .with_code("B0201")
    .with_needle(method.as_str())
    .with_label("invalid method receiver")
    .with_help("call the static method through the class name or remove the explicit receiver")
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
