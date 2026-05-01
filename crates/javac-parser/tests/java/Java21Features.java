import java.util.List;
import java.util.function.Function;

public class Java21Features {
    String switchExpr(int n) {
        return switch (n) {
            case 1 -> "one";
            case 2 -> "two";
            default -> "other";
        };
    }

    void varUsage() {
        var name = "hello";
        var x = 42;
        var flag = true;
    }

    String patternMatch(Object obj) {
        if (obj instanceof String) {
            return "string";
        }
        if (obj instanceof Integer) {
            return "integer";
        }
        return "unknown";
    }
}