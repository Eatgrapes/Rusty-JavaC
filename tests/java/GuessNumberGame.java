import java.util.Random;
import java.util.Scanner;

public class GuessNumberGame {
    public static void main(String[] args) {
        Scanner scanner = new Scanner(System.in);
        int secret = 42;
        int attempts = 0;
        boolean guessed = false;

        while (attempts < 5 && !guessed) {
            int guess = scanner.nextInt();
            attempts++;

            if (guess < secret) {
                System.out.println("too low");
            } else if (guess > secret) {
                System.out.println("too high");
            } else {
                guessed = true;
                System.out.println("correct in " + attempts + " tries");
            }
        }

        if (!guessed) {
            System.out.println("answer was " + secret);
        }
    }

    static int pickAnswer(Random random) {
        return random.nextInt(100) + 1;
    }
}
