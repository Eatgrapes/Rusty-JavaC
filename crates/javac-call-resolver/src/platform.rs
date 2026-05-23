pub const CLASSES: &[&str] = &[
    "java/io/BufferedInputStream",
    "java/io/BufferedOutputStream",
    "java/io/BufferedReader",
    "java/io/BufferedWriter",
    "java/io/Closeable",
    "java/io/EOFException",
    "java/io/File",
    "java/io/FileInputStream",
    "java/io/FileNotFoundException",
    "java/io/FileOutputStream",
    "java/io/FileReader",
    "java/io/FileWriter",
    "java/io/IOException",
    "java/io/InputStream",
    "java/io/InputStreamReader",
    "java/io/OutputStream",
    "java/io/OutputStreamWriter",
    "java/io/PrintStream",
    "java/io/PrintWriter",
    "java/io/Reader",
    "java/io/Serializable",
    "java/io/Writer",
    "java/lang/Appendable",
    "java/lang/AutoCloseable",
    "java/lang/Boolean",
    "java/lang/Byte",
    "java/lang/Character",
    "java/lang/CharSequence",
    "java/lang/Class",
    "java/lang/ClassLoader",
    "java/lang/Comparable",
    "java/lang/Double",
    "java/lang/Enum",
    "java/lang/Exception",
    "java/lang/Float",
    "java/lang/IllegalArgumentException",
    "java/lang/IllegalStateException",
    "java/lang/IndexOutOfBoundsException",
    "java/lang/Integer",
    "java/lang/Iterable",
    "java/lang/Long",
    "java/lang/Math",
    "java/lang/Number",
    "java/lang/Object",
    "java/lang/Runtime",
    "java/lang/RuntimeException",
    "java/lang/Short",
    "java/lang/String",
    "java/lang/StringBuffer",
    "java/lang/StringBuilder",
    "java/lang/System",
    "java/lang/Thread",
    "java/lang/Throwable",
    "java/lang/Void",
    "java/math/BigDecimal",
    "java/math/BigInteger",
    "java/net/InetAddress",
    "java/net/URI",
    "java/net/URL",
    "java/nio/file/FileSystems",
    "java/nio/file/Files",
    "java/nio/file/Path",
    "java/nio/file/Paths",
    "java/time/Duration",
    "java/time/Instant",
    "java/time/LocalDate",
    "java/time/LocalDateTime",
    "java/time/LocalTime",
    "java/time/Period",
    "java/time/ZoneId",
    "java/time/ZonedDateTime",
    "java/util/ArrayDeque",
    "java/util/ArrayList",
    "java/util/Arrays",
    "java/util/Collection",
    "java/util/Collections",
    "java/util/Comparator",
    "java/util/Date",
    "java/util/Deque",
    "java/util/HashMap",
    "java/util/HashSet",
    "java/util/Iterator",
    "java/util/LinkedHashMap",
    "java/util/LinkedHashSet",
    "java/util/LinkedList",
    "java/util/List",
    "java/util/Locale",
    "java/util/Map",
    "java/util/Objects",
    "java/util/Optional",
    "java/util/OptionalDouble",
    "java/util/OptionalInt",
    "java/util/OptionalLong",
    "java/util/PriorityQueue",
    "java/util/Queue",
    "java/util/Random",
    "java/util/Scanner",
    "java/util/Set",
    "java/util/StringJoiner",
    "java/util/TreeMap",
    "java/util/TreeSet",
    "java/util/UUID",
    "java/util/Vector",
    "java/util/function/BiConsumer",
    "java/util/function/BiFunction",
    "java/util/function/BinaryOperator",
    "java/util/function/Consumer",
    "java/util/function/Function",
    "java/util/function/Predicate",
    "java/util/function/Supplier",
    "java/util/function/UnaryOperator",
];

pub fn class_name(simple_name: &str) -> Option<&'static str> {
    CLASSES
        .iter()
        .copied()
        .find(|name| simple_name_of(name) == simple_name)
}

pub fn internal_class_name(internal_name: &str) -> Option<&'static str> {
    CLASSES.iter().copied().find(|name| *name == internal_name)
}

pub fn package_name(package: &str) -> bool {
    CLASSES.iter().any(|name| package_of(name) == package)
}

fn simple_name_of(internal_name: &str) -> &str {
    internal_name.rsplit('/').next().unwrap_or(internal_name)
}

fn package_of(internal_name: &str) -> &str {
    internal_name
        .rsplit_once('/')
        .map_or("", |(package, _)| package)
}
