pub const CLASSES: &[&str] = &[
    "java/util/function/BiConsumer",
    "java/util/function/BiFunction",
    "java/util/function/BinaryOperator",
    "java/util/function/Consumer",
    "java/util/function/Function",
    "java/util/function/Predicate",
    "java/util/function/Supplier",
    "java/util/function/UnaryOperator",
];

use super::{Method, Parent, parent, public_interface_method};

pub const INTERFACES: &[&str] = CLASSES;

pub const METHODS: &[Method] = &[
    public_interface_method(
        "java/util/function/BiConsumer",
        "accept",
        "(Ljava/lang/Object;Ljava/lang/Object;)V",
    ),
    public_interface_method(
        "java/util/function/BiFunction",
        "apply",
        "(Ljava/lang/Object;Ljava/lang/Object;)Ljava/lang/Object;",
    ),
    public_interface_method(
        "java/util/function/BinaryOperator",
        "apply",
        "(Ljava/lang/Object;Ljava/lang/Object;)Ljava/lang/Object;",
    ),
    public_interface_method(
        "java/util/function/Consumer",
        "accept",
        "(Ljava/lang/Object;)V",
    ),
    public_interface_method(
        "java/util/function/Function",
        "apply",
        "(Ljava/lang/Object;)Ljava/lang/Object;",
    ),
    public_interface_method(
        "java/util/function/Predicate",
        "test",
        "(Ljava/lang/Object;)Z",
    ),
    public_interface_method("java/util/function/Supplier", "get", "()Ljava/lang/Object;"),
    public_interface_method(
        "java/util/function/UnaryOperator",
        "apply",
        "(Ljava/lang/Object;)Ljava/lang/Object;",
    ),
];

pub const PARENTS: &[Parent] = &[
    parent("java/util/function/BiConsumer", "java/lang/Object"),
    parent("java/util/function/BiFunction", "java/lang/Object"),
    parent(
        "java/util/function/BinaryOperator",
        "java/util/function/BiFunction",
    ),
    parent("java/util/function/Consumer", "java/lang/Object"),
    parent("java/util/function/Function", "java/lang/Object"),
    parent("java/util/function/Predicate", "java/lang/Object"),
    parent("java/util/function/Supplier", "java/lang/Object"),
    parent(
        "java/util/function/UnaryOperator",
        "java/util/function/Function",
    ),
];
