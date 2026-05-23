package com.example.modern.flow;

public class ScopedPatternFlow {
    public static void main(String[] args) {
        var value = 2;
        switch (value) {
            case 1:
                System.out.println("one");
            case 2:
                System.out.println("two");
                break;
            default:
                System.out.println("other");
        }
    }

    Object pattern(Object obj) {
        var fallback = "unknown";
        if (obj instanceof String text) {
            return text;
        }
        if (obj instanceof Integer number) {
            return number;
        }
        return fallback;
    }
}
