package com.example;

public class SwitchExpressionCases {
    String text(String value) {
        return switch (value) {
            case "yes" -> "Y";
            case "no" -> "N";
            default -> "?";
        };
    }

    String number(int value) {
        return switch (value) {
            case 1 -> "one";
            case 2 -> "two";
            default -> "other";
        };
    }
}
