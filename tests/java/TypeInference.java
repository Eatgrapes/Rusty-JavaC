import java.util.function.Supplier;

public class TypeInference {
    public static void main(String[] args) {
        var inferredInt = 7;
        var inferredText = "score";
        var inferredArray = new int[] { 1, 2, 3 };

        long widened = inferredInt + 5L;
        double precise = widened + 0.5d;
        String message = inferredText + ":" + precise + ":" + inferredArray[1];
        Supplier<String> supplier = () -> "lambda result";

        System.out.println(message);
        System.out.println(supplier.get());
    }
}
