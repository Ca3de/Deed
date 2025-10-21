/**
 * Deed Database Java Client
 *
 * This demonstrates how to use Deed from Java, just like you would use
 * PostgreSQL (JDBC), MySQL (JDBC), or MongoDB (Java Driver).
 *
 * Requirements (Maven):
 *     <dependency>
 *         <groupId>com.google.code.gson</groupId>
 *         <artifactId>gson</artifactId>
 *         <version>2.10.1</version>
 *     </dependency>
 *
 * Or Gradle:
 *     implementation 'com.google.code.gson:gson:2.10.1'
 *
 * Usage:
 *     1. Start the REST API server:
 *        cargo run --example rest_api_server
 *
 *     2. Compile and run this file:
 *        javac JavaClient.java
 *        java JavaClient
 */

import java.net.URI;
import java.net.http.*;
import java.util.HashMap;
import java.util.Map;
import com.google.gson.*;

/**
 * Java client for Deed Database - works like JDBC for PostgreSQL/MySQL
 */
class DeedDB {
    private String url;
    private String sessionId;
    private HttpClient client;
    private Gson gson;

    /**
     * Initialize connection to Deed database
     *
     * Similar to:
     *     Connection conn = DriverManager.getConnection("jdbc:postgresql://...", user, password)
     */
    public DeedDB(String url) {
        this.url = url;
        this.client = HttpClient.newHttpClient();
        this.gson = new Gson();
        this.sessionId = null;
    }

    public DeedDB() {
        this("http://localhost:8080");
    }

    /**
     * Connect to database (login)
     *
     * Similar to:
     *     Connection conn = DriverManager.getConnection(url, user, password)
     */
    public boolean connect(String username, String password) {
        try {
            JsonObject json = new JsonObject();
            json.addProperty("username", username);
            json.addProperty("password", password);

            HttpRequest request = HttpRequest.newBuilder()
                .uri(URI.create(url + "/api/login"))
                .header("Content-Type", "application/json")
                .POST(HttpRequest.BodyPublishers.ofString(json.toString()))
                .build();

            HttpResponse<String> response = client.send(
                request,
                HttpResponse.BodyHandlers.ofString()
            );

            JsonObject result = gson.fromJson(response.body(), JsonObject.class);

            if (result.get("success").getAsBoolean()) {
                this.sessionId = result.get("session_id").getAsString();
                System.out.println("✅ Connected: " + result.get("message").getAsString());
                return true;
            } else {
                System.out.println("❌ Connection failed: " + result.get("message").getAsString());
                return false;
            }

        } catch (Exception e) {
            System.out.println("❌ Connection error: " + e.getMessage());
            return false;
        }
    }

    /**
     * Execute a DQL query
     *
     * Similar to:
     *     Statement stmt = conn.createStatement()
     *     ResultSet rs = stmt.executeQuery("SELECT * FROM users WHERE age > 25")
     */
    public JsonObject execute(String query) throws Exception {
        if (sessionId == null) {
            throw new Exception("Not connected. Call connect() first.");
        }

        try {
            JsonObject json = new JsonObject();
            json.addProperty("session_id", sessionId);
            json.addProperty("query", query);

            HttpRequest request = HttpRequest.newBuilder()
                .uri(URI.create(url + "/api/query"))
                .header("Content-Type", "application/json")
                .POST(HttpRequest.BodyPublishers.ofString(json.toString()))
                .build();

            HttpResponse<String> response = client.send(
                request,
                HttpResponse.BodyHandlers.ofString()
            );

            return gson.fromJson(response.body(), JsonObject.class);

        } catch (Exception e) {
            JsonObject error = new JsonObject();
            error.addProperty("success", false);
            error.addProperty("error", e.getMessage());
            return error;
        }
    }

    /**
     * Insert data (convenience method)
     *
     * Similar to:
     *     PreparedStatement pstmt = conn.prepareStatement("INSERT INTO users VALUES (?, ?)")
     *     pstmt.setString(1, name)
     *     pstmt.setInt(2, age)
     *     pstmt.executeUpdate()
     */
    public boolean insert(String table, Map<String, Object> data) throws Exception {
        StringBuilder props = new StringBuilder();
        int i = 0;
        for (Map.Entry<String, Object> entry : data.entrySet()) {
            if (i > 0) props.append(", ");
            String value;
            if (entry.getValue() instanceof String) {
                value = "\"" + entry.getValue() + "\"";
            } else {
                value = entry.getValue().toString();
            }
            props.append(entry.getKey()).append(": ").append(value);
            i++;
        }

        String query = "INSERT INTO " + table + " VALUES ({" + props + "})";
        JsonObject result = execute(query);
        return result.get("success").getAsBoolean();
    }

    /**
     * Select data (convenience method)
     *
     * Similar to:
     *     Statement stmt = conn.createStatement()
     *     ResultSet rs = stmt.executeQuery("SELECT name, age FROM users WHERE age > 25")
     */
    public JsonObject select(String table, String where, String fields) throws Exception {
        String query;
        if (where != null && !where.isEmpty()) {
            query = "FROM " + table + " WHERE " + where + " SELECT " + fields;
        } else {
            query = "FROM " + table + " SELECT " + fields;
        }
        return execute(query);
    }

    public JsonObject select(String table, String fields) throws Exception {
        return select(table, null, fields);
    }

    /**
     * Disconnect from database (logout)
     *
     * Similar to:
     *     conn.close()
     */
    public boolean disconnect() {
        if (sessionId == null) {
            return true;
        }

        try {
            JsonObject json = new JsonObject();
            json.addProperty("session_id", sessionId);

            HttpRequest request = HttpRequest.newBuilder()
                .uri(URI.create(url + "/api/logout"))
                .header("Content-Type", "application/json")
                .POST(HttpRequest.BodyPublishers.ofString(json.toString()))
                .build();

            HttpResponse<String> response = client.send(
                request,
                HttpResponse.BodyHandlers.ofString()
            );

            JsonObject result = gson.fromJson(response.body(), JsonObject.class);

            if (result.get("success").getAsBoolean()) {
                System.out.println("✅ Disconnected: " + result.get("message").getAsString());
                sessionId = null;
                return true;
            } else {
                System.out.println("❌ Disconnect failed: " + result.get("message").getAsString());
                return false;
            }

        } catch (Exception e) {
            System.out.println("❌ Disconnect error: " + e.getMessage());
            return false;
        }
    }

    /**
     * Demonstrate Deed Java client usage
     */
    public static void main(String[] args) throws Exception {
        System.out.println("=".repeat(60));
        System.out.println("Deed Database - Java Client Demo");
        System.out.println("=".repeat(60));
        System.out.println();

        // Create database connection
        DeedDB db = new DeedDB();

        // Connect (like JDBC DriverManager.getConnection)
        System.out.println("1. Connecting to database...");
        if (!db.connect("admin", "admin123")) {
            return;
        }
        System.out.println();

        // Query existing data
        System.out.println("2. Querying existing data...");
        JsonObject result = db.select("Users", "age > 25", "name, city");
        System.out.println("   Result: " + result);
        System.out.println();

        // Insert new data
        System.out.println("3. Inserting new user...");
        Map<String, Object> userData = new HashMap<>();
        userData.put("name", "Emily");
        userData.put("age", 32);
        userData.put("city", "Chicago");
        boolean success = db.insert("Users", userData);
        System.out.println("   Insert success: " + success);
        System.out.println();

        // Query all users
        System.out.println("4. Querying all users...");
        result = db.select("Users", "name, age, city");
        System.out.println("   Result: " + result);
        System.out.println();

        // Aggregation query
        System.out.println("5. Aggregation - count users by city...");
        result = db.execute("FROM Users SELECT city, COUNT(*) GROUP BY city");
        System.out.println("   Result: " + result);
        System.out.println();

        // Transaction example
        System.out.println("6. Transaction example (multiple inserts)...");
        db.execute("BEGIN TRANSACTION");
        Map<String, Object> product1 = new HashMap<>();
        product1.put("name", "Tablet");
        product1.put("price", 399);
        product1.put("stock", 20);
        db.insert("Products", product1);

        Map<String, Object> product2 = new HashMap<>();
        product2.put("name", "Charger");
        product2.put("price", 19);
        product2.put("stock", 100);
        db.insert("Products", product2);
        db.execute("COMMIT");
        System.out.println("   ✓ Transaction committed");
        System.out.println();

        // Complex query
        System.out.println("7. Query products under $100...");
        result = db.select("Products", "price < 100", "name, price");
        System.out.println("   Result: " + result);
        System.out.println();

        // Update example
        System.out.println("8. Update example...");
        result = db.execute("UPDATE Users SET age = 33 WHERE name = \"Emily\"");
        System.out.println("   Result: " + result);
        System.out.println();

        // Delete example
        System.out.println("9. Delete example...");
        result = db.execute("DELETE FROM Products WHERE stock < 25");
        System.out.println("   Result: " + result);
        System.out.println();

        // Disconnect
        System.out.println("10. Disconnecting...");
        db.disconnect();
        System.out.println();

        System.out.println("=".repeat(60));
        System.out.println("✅ Demo completed successfully!");
        System.out.println("=".repeat(60));
    }
}
