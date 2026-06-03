import { Request, Response, NextFunction } from 'express';
import * as admin from 'firebase-admin';

export interface AuthenticatedRequest extends Request {
  user?: admin.auth.DecodedIdToken & {
    isAdmin?: boolean;
    name?: string;
  };
}

export const authenticateToken = async (
  req: AuthenticatedRequest,
  res: Response,
  next: NextFunction
): Promise<any> => {
  const authHeader = req.headers['authorization'];
  const token = authHeader && authHeader.split(' ')[1];

  if (!token) {
    return res.status(401).json({ error: 'Access token required!' });
  }

  try {
    const decodedToken = await admin.auth().verifyIdToken(token);
    
    const db = admin.firestore();
    const userRef = db.collection('users').doc(decodedToken.uid);
    let userDoc = await userRef.get();
    
    let is_admin = false;
    let status = 1;
    let name = decodedToken.name || decodedToken.email?.split('@')[0] || 'Unknown User';

    if (!userDoc.exists) {
      // Auto-bootstrap user record in Firestore on first token validation
      console.log(`Auto-bootstrapping Firestore user profile for user: ${decodedToken.email}`);
      const isFirstUser = (await db.collection('users').limit(1).get()).empty;
      
      // Make the very first user who logs in an Admin automatically for convenience
      is_admin = isFirstUser;
      
      const newUserData = {
        name,
        email: decodedToken.email || '',
        note: is_admin ? 'Primary Administrator' : 'Auto-registered User',
        status: 1,
        is_admin,
        created_at: admin.firestore.FieldValue.serverTimestamp()
      };
      
      await userRef.set(newUserData);
      userDoc = await userRef.get();
    } else {
      const userData = userDoc.data();
      is_admin = userData?.is_admin === true;
      status = userData?.status ?? 1;
      name = userData?.name || name;
    }

    if (status === 0) {
      return res.status(403).json({ error: 'User account is disabled!' });
    }

    req.user = {
      ...decodedToken,
      isAdmin: is_admin,
      name: name
    };
    
    next();
  } catch (error: any) {
    console.error('Token verification failed:', error);
    return res.status(401).json({ error: 'Invalid or expired access token!' });
  }
};
