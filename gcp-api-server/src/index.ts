import express from 'express';
import cors from 'cors';
import dotenv from 'dotenv';
import * as admin from 'firebase-admin';
import apiRouter from './routes/api';

dotenv.config();

const port = process.env.PORT || 8080;

// Initialize Firebase Admin SDK
try {
  if (admin.apps.length === 0) {
    // If running locally, check if GOOGLE_APPLICATION_CREDENTIALS points to service account
    if (process.env.GOOGLE_APPLICATION_CREDENTIALS) {
      console.log(`Initializing Firebase Admin using credentials from: ${process.env.GOOGLE_APPLICATION_CREDENTIALS}`);
    }
    admin.initializeApp();
    console.log("Firebase Admin initialized successfully.");
  }
} catch (error) {
  console.error("Firebase Admin Initialization Error:", error);
}

const app = express();

// Apply global middleware
app.use(cors());
app.use(express.json());

// Mount the RustDesk API router (Prefix all endpoints with /api as expected by the client)
app.use('/api', apiRouter);

// Health check endpoint (for Google Cloud Run probes)
app.get('/health', (req, res) => {
  res.json({
    status: 'ok',
    timestamp: new Date().toISOString(),
    project: admin.apps[0]?.options.projectId || 'uninitialized'
  });
});

// Catch-all route
app.use((req, res) => {
  res.status(404).json({ error: `Route not found: ${req.method} ${req.path}` });
});

// Start HTTP server
app.listen(port, () => {
  console.log(`================================================================`);
  console.log(`🚀 RustDesk Custom API Server is listening on port ${port}`);
  console.log(`🚀 Health probe available at: http://localhost:${port}/health`);
  console.log(`================================================================`);
});
