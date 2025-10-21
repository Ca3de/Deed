#!/usr/bin/env node
/**
 * Deed Database Node.js Client
 *
 * This demonstrates how to use Deed from Node.js, just like you would use
 * PostgreSQL (pg), MySQL (mysql2), or MongoDB (mongodb).
 *
 * Requirements:
 *     npm install axios
 *
 * Usage:
 *     1. Start the REST API server:
 *        cargo run --example rest_api_server
 *
 *     2. Run this script:
 *        node examples/nodejs_client.js
 */

const axios = require('axios');

/**
 * Node.js client for Deed Database - works like 'pg' for PostgreSQL
 */
class DeedDB {
  constructor(url = 'http://localhost:8080') {
    this.url = url;
    this.sessionId = null;
  }

  /**
   * Connect to database (login)
   *
   * Similar to:
   *     const client = new Client({ user: 'postgres', password: 'secret' })
   *     await client.connect()
   */
  async connect(username, password) {
    try {
      const response = await axios.post(`${this.url}/api/login`, {
        username,
        password
      });

      const data = response.data;

      if (data.success) {
        this.sessionId = data.session_id;
        console.log(`✅ Connected: ${data.message}`);
        return true;
      } else {
        console.log(`❌ Connection failed: ${data.message}`);
        return false;
      }

    } catch (error) {
      console.log(`❌ Connection error: ${error.message}`);
      return false;
    }
  }

  /**
   * Execute a DQL query
   *
   * Similar to:
   *     await client.query('SELECT * FROM users WHERE age > 25')
   */
  async execute(query) {
    if (!this.sessionId) {
      throw new Error('Not connected. Call connect() first.');
    }

    try {
      const response = await axios.post(`${this.url}/api/query`, {
        session_id: this.sessionId,
        query
      });

      return response.data;

    } catch (error) {
      return { success: false, error: error.message };
    }
  }

  /**
   * Insert data (convenience method)
   *
   * Similar to:
   *     await client.query('INSERT INTO users VALUES ($1, $2)', [name, age])
   */
  async insert(table, data) {
    // Convert JS object to DQL format
    const props = Object.entries(data)
      .map(([k, v]) => typeof v === 'string' ? `${k}: "${v}"` : `${k}: ${v}`)
      .join(', ');

    const query = `INSERT INTO ${table} VALUES ({${props}})`;

    const result = await this.execute(query);
    return result.success || false;
  }

  /**
   * Select data (convenience method)
   *
   * Similar to:
   *     await client.query('SELECT name, age FROM users WHERE age > 25')
   */
  async select(table, { where = null, fields = '*' } = {}) {
    let query;
    if (where) {
      query = `FROM ${table} WHERE ${where} SELECT ${fields}`;
    } else {
      query = `FROM ${table} SELECT ${fields}`;
    }

    return await this.execute(query);
  }

  /**
   * Disconnect from database (logout)
   *
   * Similar to:
   *     await client.end()
   */
  async disconnect() {
    if (!this.sessionId) {
      return true;
    }

    try {
      const response = await axios.post(`${this.url}/api/logout`, {
        session_id: this.sessionId
      });

      const data = response.data;

      if (data.success) {
        console.log(`✅ Disconnected: ${data.message}`);
        this.sessionId = null;
        return true;
      } else {
        console.log(`❌ Disconnect failed: ${data.message}`);
        return false;
      }

    } catch (error) {
      console.log(`❌ Disconnect error: ${error.message}`);
      return false;
    }
  }
}

/**
 * Demonstrate Deed Node.js client usage
 */
async function main() {
  console.log('='.repeat(60));
  console.log('Deed Database - Node.js Client Demo');
  console.log('='.repeat(60));
  console.log();

  // Create database connection
  const db = new DeedDB();

  try {
    // Connect (like pg.Client.connect or mysql.createConnection)
    console.log('1. Connecting to database...');
    if (!await db.connect('admin', 'admin123')) {
      return;
    }
    console.log();

    // Query existing data
    console.log('2. Querying existing data...');
    let result = await db.select('Users', {
      where: 'age > 25',
      fields: 'name, city'
    });
    console.log(`   Result: ${JSON.stringify(result, null, 2)}`);
    console.log();

    // Insert new data
    console.log('3. Inserting new user...');
    const success = await db.insert('Users', {
      name: 'David',
      age: 28,
      city: 'Seattle'
    });
    console.log(`   Insert success: ${success}`);
    console.log();

    // Query all users
    console.log('4. Querying all users...');
    result = await db.select('Users', { fields: 'name, age, city' });
    console.log(`   Result: ${JSON.stringify(result, null, 2)}`);
    console.log();

    // Aggregation query
    console.log('5. Aggregation - count users by city...');
    result = await db.execute('FROM Users SELECT city, COUNT(*) GROUP BY city');
    console.log(`   Result: ${JSON.stringify(result, null, 2)}`);
    console.log();

    // Transaction example
    console.log('6. Transaction example (multiple inserts)...');
    await db.execute('BEGIN TRANSACTION');
    await db.insert('Products', { name: 'Webcam', price: 89, stock: 25 });
    await db.insert('Products', { name: 'Headset', price: 59, stock: 40 });
    await db.execute('COMMIT');
    console.log('   ✓ Transaction committed');
    console.log();

    // Complex query
    console.log('7. Query products under $100...');
    result = await db.select('Products', {
      where: 'price < 100',
      fields: 'name, price'
    });
    console.log(`   Result: ${JSON.stringify(result, null, 2)}`);
    console.log();

    // Update example
    console.log('8. Update example...');
    result = await db.execute('UPDATE Users SET age = 29 WHERE name = "David"');
    console.log(`   Result: ${JSON.stringify(result, null, 2)}`);
    console.log();

    // Delete example
    console.log('9. Delete example...');
    result = await db.execute('DELETE FROM Products WHERE stock < 30');
    console.log(`   Result: ${JSON.stringify(result, null, 2)}`);
    console.log();

    // Disconnect
    console.log('10. Disconnecting...');
    await db.disconnect();
    console.log();

    console.log('='.repeat(60));
    console.log('✅ Demo completed successfully!');
    console.log('='.repeat(60));

  } catch (error) {
    console.error(`❌ Error: ${error.message}`);
  }
}

// Run the demo
main();
