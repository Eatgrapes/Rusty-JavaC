import java.util.function.Consumer;

public class LambdaConsumerTest {
    public static void main(String[] args) {
        Consumer<String> c = x -> System.out.println(x);
        c.accept("hello from consumer");
    }
}
