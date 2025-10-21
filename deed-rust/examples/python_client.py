#!/usr/bin/env python3
"""
Deed Database Python Client

This demonstrates how to use Deed from Python, just like you would use
PostgreSQL (psycopg2), MySQL (mysql-connector), or MongoDB (pymongo).

Requirements:
    pip install requests

Usage:
    1. Start the REST API server:
       cargo run --example rest_api_server

    2. Run this script:
       python examples/python_client.py
"""

import requests
import json
from typing import Optional, Dict, Any, List


class DeedDB:
    """Python client for Deed Database - works like psycopg2 for PostgreSQL"""

    def __init__(self, url: str = "http://localhost:8080"):
        """Initialize connection to Deed database"""
        self.url = url
        self.session_id: Optional[str] = None

    def connect(self, username: str, password: str) -> bool:
        """
        Connect to database (login)

        Similar to:
            conn = psycopg2.connect(dbname="mydb", user="postgres", password="secret")
        """
        try:
            response = requests.post(
                f"{self.url}/api/login",
                json={"username": username, "password": password},
                headers={"Content-Type": "application/json"}
            )
            data = response.json()

            if data["success"]:
                self.session_id = data["session_id"]
                print(f"✅ Connected: {data['message']}")
                return True
            else:
                print(f"❌ Connection failed: {data['message']}")
                return False

        except Exception as e:
            print(f"❌ Connection error: {e}")
            return False

    def execute(self, query: str) -> Dict[str, Any]:
        """
        Execute a DQL query

        Similar to:
            cursor.execute("SELECT * FROM users WHERE age > 25")
        """
        if not self.session_id:
            raise Exception("Not connected. Call connect() first.")

        try:
            response = requests.post(
                f"{self.url}/api/query",
                json={"session_id": self.session_id, "query": query},
                headers={"Content-Type": "application/json"}
            )
            return response.json()

        except Exception as e:
            return {"success": False, "error": str(e)}

    def insert(self, table: str, data: Dict[str, Any]) -> bool:
        """
        Insert data (convenience method)

        Similar to:
            cursor.execute("INSERT INTO users VALUES (%s, %s)", (name, age))
        """
        # Convert Python dict to DQL format
        props = ", ".join([f"{k}: \"{v}\"" if isinstance(v, str) else f"{k}: {v}"
                          for k, v in data.items()])
        query = f"INSERT INTO {table} VALUES ({{{props}}})"

        result = self.execute(query)
        return result.get("success", False)

    def select(self, table: str, where: Optional[str] = None,
              fields: str = "*") -> Dict[str, Any]:
        """
        Select data (convenience method)

        Similar to:
            cursor.execute("SELECT name, age FROM users WHERE age > 25")
        """
        if where:
            query = f"FROM {table} WHERE {where} SELECT {fields}"
        else:
            query = f"FROM {table} SELECT {fields}"

        return self.execute(query)

    def disconnect(self) -> bool:
        """
        Disconnect from database (logout)

        Similar to:
            conn.close()
        """
        if not self.session_id:
            return True

        try:
            response = requests.post(
                f"{self.url}/api/logout",
                json={"session_id": self.session_id},
                headers={"Content-Type": "application/json"}
            )
            data = response.json()

            if data["success"]:
                print(f"✅ Disconnected: {data['message']}")
                self.session_id = None
                return True
            else:
                print(f"❌ Disconnect failed: {data['message']}")
                return False

        except Exception as e:
            print(f"❌ Disconnect error: {e}")
            return False


def main():
    """Demonstrate Deed Python client usage"""

    print("=" * 60)
    print("Deed Database - Python Client Demo")
    print("=" * 60)
    print()

    # Create database connection
    db = DeedDB()

    # Connect (like psycopg2.connect or mysql.connector.connect)
    print("1. Connecting to database...")
    if not db.connect("admin", "admin123"):
        return
    print()

    # Query existing data
    print("2. Querying existing data...")
    result = db.select("Users", where="age > 25", fields="name, city")
    print(f"   Result: {json.dumps(result, indent=2)}")
    print()

    # Insert new data
    print("3. Inserting new user...")
    success = db.insert("Users", {
        "name": "Charlie",
        "age": 35,
        "city": "Boston"
    })
    print(f"   Insert success: {success}")
    print()

    # Query all users
    print("4. Querying all users...")
    result = db.select("Users", fields="name, age, city")
    print(f"   Result: {json.dumps(result, indent=2)}")
    print()

    # Aggregation query
    print("5. Aggregation - count users by city...")
    result = db.execute("FROM Users SELECT city, COUNT(*) GROUP BY city")
    print(f"   Result: {json.dumps(result, indent=2)}")
    print()

    # Transaction example
    print("6. Transaction example (multiple inserts)...")
    db.execute("BEGIN TRANSACTION")
    db.insert("Products", {"name": "Keyboard", "price": 79, "stock": 30})
    db.insert("Products", {"name": "Monitor", "price": 299, "stock": 15})
    db.execute("COMMIT")
    print("   ✓ Transaction committed")
    print()

    # Complex query
    print("7. Query products under $100...")
    result = db.select("Products", where="price < 100", fields="name, price")
    print(f"   Result: {json.dumps(result, indent=2)}")
    print()

    # Update example
    print("8. Update example...")
    result = db.execute('UPDATE Users SET age = 36 WHERE name = "Charlie"')
    print(f"   Result: {json.dumps(result, indent=2)}")
    print()

    # Delete example
    print("9. Delete example...")
    result = db.execute('DELETE FROM Products WHERE stock < 20')
    print(f"   Result: {json.dumps(result, indent=2)}")
    print()

    # Disconnect
    print("10. Disconnecting...")
    db.disconnect()
    print()

    print("=" * 60)
    print("✅ Demo completed successfully!")
    print("=" * 60)


if __name__ == "__main__":
    main()
