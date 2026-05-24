import java.util.function.Supplier;

public class LambdaTest {
    public static void main(String[] args) {
        Supplier<String> r = () -> "hello from lambda";
        System.out.println(r.get());
    }
}
