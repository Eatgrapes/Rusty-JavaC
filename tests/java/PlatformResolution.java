import java.util.ArrayList;
import java.util.HashMap;
import java.util.Optional;
import java.util.function.Consumer;
import java.util.function.Function;
import java.util.function.Predicate;
import java.util.function.Supplier;

public class PlatformResolution {
    void calls(String text, Object obj, Exception exception) {
        int textHash = text.hashCode();
        boolean same = text.equals(obj);
        System.out.println(textHash);
        System.out.println(same);
        System.out.println(obj);
        exception.printStackTrace();
    }

    void collectionCalls(String text, Optional optional) {
        ArrayList list = new ArrayList();
        boolean added = list.add(text);
        Object first = list.get(0);

        HashMap map = new HashMap();
        Object previous = map.put("key", text);
        Object value = map.get("key");

        System.out.println(added);
        System.out.println(first);
        System.out.println(previous);
        System.out.println(value);

        if (optional.isPresent()) {
            System.out.println(optional.get());
        }
    }

    void functionCalls(
        Supplier supplier,
        Consumer consumer,
        Function function,
        Predicate predicate
    ) {
        Object supplied = supplier.get();
        consumer.accept(supplied);
        Object applied = function.apply(supplied);
        boolean accepted = predicate.test(applied);
        System.out.println(accepted);
    }
}
