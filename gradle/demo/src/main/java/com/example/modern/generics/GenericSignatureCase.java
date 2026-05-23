package com.example.modern.generics;

public class GenericSignatureCase<T> {
    T id(T value) {
        return value;
    }

    Object choose(Object obj) {
        if (obj instanceof String text) {
            return text;
        }
        return obj;
    }
}
