import { Request, Response } from 'express';
import * as admin from 'firebase-admin';
import { AuthenticatedRequest } from '../middleware/auth';

/**
 * Handles RustDesk client login requests by checking credentials against Firebase Auth REST API.
 */
export const login = async (req: Request, res: Response): Promise<any> => {
  const { username, password, id, uuid, deviceInfo } = req.body;

  if (!username || !password) {
    return res.status(400).json({ error: 'Username and password are required' });
  }

  const apiKey = process.env.FIREBASE_API_KEY;
  if (!apiKey) {
    console.error('FIREBASE_API_KEY is not defined in the environment variables');
    return res.status(500).json({ error: 'Server authentication configuration error.' });
  }

  try {
    // Authenticate with Firebase Auth REST API
    const authUrl = `https://identitytoolkit.googleapis.com/v1/accounts:signInWithPassword?key=${apiKey}`;
    const authResponse = await fetch(authUrl, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({
        email: username,
        password: password,
        returnSecureToken: true,
      }),
    });

    const authData: any = await authResponse.json();

    if (!authResponse.ok) {
      const fbError = authData?.error?.message;
      let userFriendlyMessage = 'Authentication failed';
      if (fbError === 'INVALID_PASSWORD' || fbError === 'EMAIL_NOT_FOUND') {
        userFriendlyMessage = 'Invalid email or password';
      } else if (fbError === 'USER_DISABLED') {
        userFriendlyMessage = 'This account has been disabled';
      }
      return res.status(401).json({ error: userFriendlyMessage });
    }

    const { idToken, localId, email } = authData;

    // Retrieve or bootstrap user information in Cloud Firestore
    const db = admin.firestore();
    const userRef = db.collection('users').doc(localId);
    let userDoc = await userRef.get();

    let is_admin = false;
    let status = 1;
    let name = email.split('@')[0];

    if (!userDoc.exists) {
      // Determine if this is the first user registered in Firestore to grant Admin permissions
      const isFirstUser = (await db.collection('users').limit(1).get()).empty;
      is_admin = isFirstUser;

      const newUserData = {
        name,
        email,
        note: is_admin ? 'Primary Administrator' : 'Auto-registered User',
        status: 1,
        is_admin,
        created_at: admin.firestore.FieldValue.serverTimestamp(),
      };

      await userRef.set(newUserData);
      console.log(`Auto-bootstrapped Firestore user profile on login for: ${email}`);
    } else {
      const userData = userDoc.data();
      is_admin = userData?.is_admin === true;
      status = userData?.status ?? 1;
      name = userData?.name || name;
    }

    if (status === 0) {
      return res.status(403).json({ error: 'This user account is disabled' });
    }

    // Capture and log login event details (RustDesk client details)
    console.log(`User logged in successfully: ${email} from client: ${id} (OS: ${deviceInfo?.os || 'Unknown'})`);

    // Write client device info log to active connection track or device logging in Firestore
    if (id && uuid) {
      await db.collection('user_devices').doc(`${localId}_${id}`).set({
        uid: localId,
        deviceId: id,
        deviceUuid: uuid,
        deviceInfo: deviceInfo || {},
        last_login: admin.firestore.FieldValue.serverTimestamp(),
      }, { merge: true });
    }

    // Respond in the format RustDesk Client expects
    return res.json({
      access_token: idToken,
      type: 'access_token',
      user: {
        name,
        email,
        note: userDoc.exists ? userDoc.data()?.note || '' : '',
        status,
        is_admin,
      },
    });
  } catch (error: any) {
    console.error('Error during authentication process:', error);
    return res.status(500).json({ error: 'Internal Server Error' });
  }
};

/**
 * Handle user logout.
 */
export const logout = async (req: Request, res: Response): Promise<any> => {
  const { id, uuid } = req.body;
  console.log(`Client logged out: ID=${id}, UUID=${uuid}`);
  return res.json({});
};

/**
 * Returns current authenticated user state.
 */
export const getCurrentUser = async (req: AuthenticatedRequest, res: Response): Promise<any> => {
  if (!req.user) {
    return res.status(401).json({ error: 'Unauthorized' });
  }

  try {
    const db = admin.firestore();
    const userDoc = await db.collection('users').doc(req.user.uid).get();

    if (!userDoc.exists) {
      return res.json({
        name: req.user.name || '',
        email: req.user.email || '',
        note: '',
        status: 1,
        is_admin: req.user.isAdmin || false,
      });
    }

    const userData = userDoc.data();
    return res.json({
      name: userData?.name || req.user.name || '',
      email: userData?.email || req.user.email || '',
      note: userData?.note || '',
      status: userData?.status ?? 1,
      is_admin: userData?.is_admin === true,
    });
  } catch (error) {
    console.error('Error fetching current user:', error);
    return res.status(500).json({ error: 'Internal Server Error' });
  }
};

/**
 * Endpoint to serve OIDC options to RustDesk client.
 * Returning empty array disables SSO/OIDC buttons and keeps standard password field active.
 */
export const getLoginOptions = async (req: Request, res: Response): Promise<any> => {
  return res.json([]);
};
