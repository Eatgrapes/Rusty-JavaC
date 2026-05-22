import java.io.IOException;

public class ControlFlow {
    void ifElse(int x) {
        if (x > 0) {
            System.out.println("positive");
        } else if (x < 0) {
            System.out.println("negative");
        } else {
            System.out.println("zero");
        }
    }

    void loops() {
        for (int i = 0; i < 10; i++) {
            System.out.println(i);
        }

        int[] arr = new int[]{1, 2, 3};
        for (int v : arr) {
            System.out.println(v);
        }

        while (true) {
            break;
        }

        do {
            continue;
        } while (false);
    }

    String switchExpr(int n) {
        return switch (n) {
            case 1 -> "one";
            case 2 -> "two";
            default -> "other";
        };
    }

    void switchStmt(int n) {
        switch (n) {
            case 1:
                System.out.println("one");
                break;
            case 2:
                System.out.println("two");
                break;
            default:
                System.out.println("other");
        }
    }

    void tryCatch() {
        try {
            risky();
        } catch (IOException e) {
            e.printStackTrace();
        } finally {
            cleanup();
        }
    }

    void tryWithResources() throws IOException {
        try (var stream = new java.io.FileInputStream("test.txt")) {
            stream.read();
        }
    }

    native void risky() throws IOException;
    void cleanup() {}
}
