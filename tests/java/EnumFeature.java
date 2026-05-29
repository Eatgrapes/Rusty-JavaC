public enum EnumFeature {
    RED(1),
    BLUE(2);

    private final int code;

    EnumFeature(int code) {
        this.code = code;
    }

    int code() {
        return code;
    }

    public static void main(String[] args) {
        EnumFeature first = EnumFeature.RED;
        System.out.println(first.name());
        System.out.println(first.ordinal());
        System.out.println(first.code());
        System.out.println(EnumFeature.values().length);
        System.out.println(EnumFeature.valueOf("BLUE").code());
    }
}
