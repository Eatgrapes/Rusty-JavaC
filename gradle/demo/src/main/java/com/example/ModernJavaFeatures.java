package com.example;

import java.util.List;
import java.util.function.Function;

public class ModernJavaFeatures {
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

    Object patternMatch(Object obj) {
        if (obj instanceof String text) {
            return text;
        }
        if (obj instanceof Integer number) {
            return number;
        }
        return "unknown";
    }
}
