# Custom GCP RustDesk API Server (Express + Firebase)

This directory contains a custom API server for the RustDesk client built using Node.js, TypeScript, and Google Cloud Platform. It replaces commercial or third-party RustDesk API backends with a secure, serverless solution.

---

## 🛠️ Tech Stack & Services
1. **Google Cloud Run**: Serverless container hosting with auto-scaling (scales down to 0, minimizing costs).
2. **Firebase Authentication**: For user login verification and issuing secure JWT access tokens.
3. **Cloud Firestore**: Real-time database for storing address books, client devices, and device groups.

---

## 🏗️ Firebase Console Configuration Steps

Before running or deploying the server, you need to configure your Firebase project:

1. **Create/Open Firebase Project**:
   - Go to [Firebase Console](https://console.firebase.google.com/) and open your project (e.g., `cislink-500-takeaway`).

2. **Enable Email/Password Authentication**:
   - Navigate to **Authentication** -> **Sign-in method**.
   - Click **Add new provider**, select **Email/Password**, enable it, and save.
   - Register your primary admin user email and password here.

3. **Enable Firestore Database**:
   - Navigate to **Firestore Database** and click **Create database**.
   - Select a region (e.g., `europe-west4` to keep latency low).
   - Start in **Production mode** (you will connect using the Admin SDK, which bypasses client rules).

4. **Retrieve Web API Key**:
   - Navigate to **Project Settings** (gear icon in the top left) -> **General**.
   - Copy the **Web API Key**. This will be used as `FIREBASE_API_KEY` in environment variables.

5. **Generate Service Account Key (For Local Run)**:
   - Navigate to **Project Settings** -> **Service accounts**.
   - Click **Generate new private key**.
   - Save the downloaded JSON file as `service-account.json` inside this folder.
   - ⚠️ **DO NOT commit this key file to git!** It is already in the project's `.gitignore` or ignored patterns.

---

## 💻 Running the Server Locally

1. **Install Dependencies**:
   ```bash
   cd D:\Rustdesk\gcp-api-server
   npm install
   ```

2. **Configure `.env` File**:
   Update `.env` in this directory:
   ```env
   PORT=8080
   FIREBASE_PROJECT_ID=cislink-500-takeaway
   FIREBASE_API_KEY=AIzaSyA... (your Firebase Web API Key)
   GOOGLE_APPLICATION_CREDENTIALS=./service-account.json
   ```

3. **Start Development Server**:
   ```bash
   npm run dev
   ```
   The local server will start, listening on `http://localhost:8080`.

---

## 🚀 Deploying to Google Cloud Run

To build and deploy the container directly to GCP, run these commands from the `gcp-api-server` directory using the `gcloud` CLI:

1. **Set Active Project**:
   ```bash
   gcloud config set project cislink-500-takeaway
   ```

2. **Submit Container Image to Google Container Registry (GCR)**:
   ```bash
   gcloud builds submit --tag gcr.io/cislink-500-takeaway/rustdesk-api-server
   ```

3. **Deploy Container to Cloud Run**:
   ```bash
   gcloud run deploy rustdesk-api-server \
     --image gcr.io/cislink-500-takeaway/rustdesk-api-server \
     --platform managed \
     --region europe-west4 \
     --allow-unauthenticated \
     --set-env-vars FIREBASE_PROJECT_ID=cislink-500-takeaway,FIREBASE_API_KEY=YOUR_FIREBASE_API_KEY_HERE
   ```
   *Note: On Cloud Run, Firestore/Firebase permissions are authorized automatically via the Cloud Run default service account. You do not need to configure `GOOGLE_APPLICATION_CREDENTIALS`.*

---

## 📱 Configuring RustDesk Client to use the API

To point your custom-built RustDesk installer or existing clients to your new Cloud Run server:

1. Open RustDesk.
2. Go to **Settings** -> **Network**.
3. Under the **API Server** input field, enter the Cloud Run service URL:
   `https://rustdesk-api-server-xxxx-ew.a.run.app`
4. Now, click on **Login** in the top left, enter your Firebase registered credentials, and access your group lists!
