import java.util.Random;

public class StaticStateBattle {
    static int heroHp = 80;
    static int monsterHp = 90;

    public static void main(String[] args) {
        Random random = new Random();
        int turn = 1;

        while (heroHp > 0 && monsterHp > 0 && turn <= 3) {
            switch (turn) {
                case 1:
                    int strike = random.nextInt(12) + 4;
                    monsterHp -= strike;
                    break;
                case 2:
                    int potion = random.nextInt(8) + 3;
                    heroHp += potion;
                    if (heroHp > 80) {
                        heroHp = 80;
                    }
                    break;
                default:
                    heroHp--;
                    break;
            }

            if (monsterHp <= 0) {
                break;
            }

            int hit = random.nextInt(10) + 2;
            heroHp -= hit;
            turn++;
        }

        System.out.println(heroHp);
        System.out.println(monsterHp);
    }
}
