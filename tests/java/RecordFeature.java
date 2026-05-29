public record RecordFeature(String name, int count) {
    public RecordFeature {
        if (count < 0) {
            throw new IllegalArgumentException();
        }
    }

    public String label() {
        return name + ":" + count;
    }

    public static void main(String[] args) {
        RecordFeature value = new RecordFeature("box", 3);
        System.out.println(value.name());
        System.out.println(value.count());
        System.out.println(value.label());
    }
}
