public class Expressions {
    int field = 10;

    void allExpressions() {
        int x = 42;
        long l = 100L;
        float f = 1.5f;
        double d = 2.5;
        boolean b = true;
        char c = 'z';
        String s = "text";
        Object n = null;

        int neg = -x;
        boolean not = !b;
        int bitNot = ~x;
        int inc = ++x;
        int dec = --x;
        int postInc = x++;
        int postDec = x--;

        int add = x + 1;
        int sub = x - 1;
        int mul = x * 2;
        int div = x / 2;
        int rem = x % 3;
        int shift = x << 1;
        int rshift = x >> 1;
        int urshift = x >>> 1;
        int and = x & 0xFF;
        int or = x | 0xF0;
        int xor = x ^ 0xAA;
        boolean land = b && true;
        boolean lor = b || false;

        boolean eq = x == 1;
        boolean ne = x != 1;
        boolean lt = x < 1;
        boolean gt = x > 1;
        boolean le = x <= 1;
        boolean ge = x >= 1;

        String ternary = b ? "yes" : "no";

        x += 1;
        x -= 1;
        x *= 2;
        x /= 2;
        x %= 3;
        x &= 0xFF;
        x |= 0xF0;
        x ^= 0xAA;
        x <<= 1;
        x >>= 1;
        x >>>= 1;
        x = 0;

        Object obj = (String) s;
        boolean isStr = obj instanceof String;

        int[] arr = new int[10];
        int elem = arr[0];
        String str = new String("hello");

        int len = str.length();
        char ch = "hello".charAt(0);

        this.field = 20;
        super.hashCode();
    }
}