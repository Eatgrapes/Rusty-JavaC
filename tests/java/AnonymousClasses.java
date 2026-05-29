public class AnonymousClasses {
    private static final int KEY = 7;

    private String prefix = "outer";

    public static void main(String[] args) {
        Object boxed = KEY;
        Object staticAnon = new Object() {
            public String toString() {
                return "static:" + KEY;
            }
        };

        System.out.println(boxed);
        System.out.println(staticAnon.toString());
        new AnonymousClasses().run();
    }

    void run() {
        Object instanceAnon = new Object() {
            public String toString() {
                return prefix + ":" + KEY;
            }
        };

        System.out.println(instanceAnon.toString());
    }
}
