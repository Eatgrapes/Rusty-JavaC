package com.example;

public class Classes {
    static int count = 0;

    private String name;
    protected int value;
    public final double PI = 3.14159;

    public Classes(String name, int value) {
        this.name = name;
        this.value = value;
        count++;
    }

    public String getName() {
        return name;
    }

    public void setName(String name) {
        this.name = name;
    }

    @Override
    public String toString() {
        return name + "=" + value;
    }

    static class Nested {
        int x;
    }

    abstract static class Base {
        abstract void doSomething();
    }

    interface Printable {
        void print();
        default void printTwice() {
            print();
            print();
        }
    }
}

enum Color {
    RED("#FF0000"),
    GREEN("#00FF00"),
    BLUE("#0000FF");

    private final String hex;

    Color(String hex) {
        this.hex = hex;
    }

    String hex() { return hex; }
}

record Point(int x, int y) {
    public Point {
        if (x < 0 || y < 0) {
            throw new IllegalArgumentException();
        }
    }
}