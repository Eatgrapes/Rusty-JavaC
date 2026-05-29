@FeatureMarker("demo")
public class CustomAnnotationFeature {
    public static void main(String[] args) {
        System.out.println("annotation");
    }
}

@interface FeatureMarker {
    String value();
}
