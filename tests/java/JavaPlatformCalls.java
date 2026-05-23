import java.io.BufferedReader;
import java.io.BufferedWriter;
import java.io.File;
import java.io.FileInputStream;
import java.io.IOException;
import java.io.PrintWriter;
import java.math.BigDecimal;
import java.math.BigInteger;
import java.net.URI;
import java.net.URL;
import java.nio.file.Files;
import java.nio.file.Path;
import java.nio.file.Paths;
import java.time.Duration;
import java.time.Instant;
import java.time.LocalDate;
import java.time.LocalDateTime;
import java.time.ZoneId;
import java.util.ArrayDeque;
import java.util.ArrayList;
import java.util.Arrays;
import java.util.Collection;
import java.util.HashMap;
import java.util.HashSet;
import java.util.List;
import java.util.Map;
import java.util.Optional;
import java.util.Set;
import java.util.UUID;
import java.util.function.Consumer;
import java.util.function.Function;
import java.util.function.Predicate;
import java.util.function.Supplier;

public class JavaPlatformCalls {
    BufferedReader reader;
    BufferedWriter writer;
    File file;
    PrintWriter printWriter;
    BigDecimal decimal;
    BigInteger integer;
    URI uri;
    URL url;
    Files files;
    Path path;
    Paths paths;
    LocalDate date;
    LocalDateTime dateTime;
    Instant instant;
    Duration duration;
    ZoneId zone;
    ArrayDeque queue;
    ArrayList arrayList;
    Arrays arrays;
    Collection collection;
    HashMap hashMap;
    HashSet hashSet;
    List list;
    Map map;
    Optional optional;
    Set set;
    UUID uuid;
    Consumer consumer;
    Function function;
    Predicate predicate;
    Supplier supplier;

    void stringCalls(String text) {
        int len = text.length();
        char first = text.charAt(0);
        System.out.println(text);
    }

    int objectCalls(Object obj) {
        return obj.hashCode();
    }

    void throwableCalls(Throwable throwable) {
        throwable.printStackTrace();
    }

    int fileInputCalls(String path) throws IOException {
        FileInputStream stream = new FileInputStream(path);
        int value = stream.read();
        stream.close();
        return value;
    }
}
