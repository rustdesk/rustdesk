import { Router, Response } from 'express';
import * as authController from '../controllers/authController';
import * as abController from '../controllers/abController';
import * as groupController from '../controllers/groupController';
import { authenticateToken, AuthenticatedRequest } from '../middleware/auth';
import * as admin from 'firebase-admin';

const router = Router();

// ==========================================
// Authentication Routes
// ==========================================
router.post('/login', authController.login);
router.post('/logout', authController.logout);
router.get('/login-options', authController.getLoginOptions);
router.post('/currentUser', authenticateToken as any, authController.getCurrentUser as any);

// ==========================================
// Address Book (ab) Settings & Provisioning
// ==========================================
router.get('/ab/settings', authenticateToken as any, abController.getAbSettings as any);
router.post('/ab/personal', authenticateToken as any, abController.getPersonalAb as any);

// ==========================================
// Peer Device Operations (within an Address Book)
// ==========================================
router.post('/ab/peers', authenticateToken as any, abController.getPeers as any);
router.post('/ab/peer/add/:guid', authenticateToken as any, abController.addPeer as any);
router.put('/ab/peer/update/:guid', authenticateToken as any, abController.updatePeer as any);
router.delete('/ab/peer/:guid', authenticateToken as any, abController.deletePeers as any);

// ==========================================
// Tag Configuration within an Address Book
// ==========================================
router.post('/ab/tags/:guid', authenticateToken as any, abController.getTags as any);
router.post('/ab/tag/add/:guid', authenticateToken as any, abController.addTag as any);
router.put('/ab/tag/rename/:guid', authenticateToken as any, abController.renameTag as any);
router.put('/ab/tag/update/:guid', authenticateToken as any, abController.updateTag as any);
router.delete('/ab/tag/:guid', authenticateToken as any, abController.deleteTag as any);

// ==========================================
// Device Group Operations
// ==========================================
router.get('/device-group/accessible', authenticateToken as any, groupController.getAccessibleDeviceGroups as any);

// ==========================================
// Direct Users & Direct Peers Retrieval (Global list checks)
// ==========================================
router.get('/users', authenticateToken as any, async (req: AuthenticatedRequest, res: Response): Promise<any> => {
  if (!req.user) {
    return res.status(401).json({ error: 'Unauthorized' });
  }

  const current = parseInt(req.query.current as string) || 1;
  const pageSize = parseInt(req.query.pageSize as string) || 100;

  try {
    const db = admin.firestore();
    let allUsers: any[] = [];

    if (req.user.isAdmin) {
      const querySnapshot = await db.collection('users').get();
      allUsers = querySnapshot.docs.map(doc => {
        const data = doc.data();
        return {
          name: data.name,
          email: data.email,
          note: data.note || '',
          status: data.status ?? 1,
          is_admin: data.is_admin === true
        };
      });
    } else {
      const doc = await db.collection('users').doc(req.user.uid).get();
      if (doc.exists) {
        const data = doc.data();
        allUsers = [{
          name: data?.name || req.user.name,
          email: data?.email || req.user.email,
          note: data?.note || '',
          status: data?.status ?? 1,
          is_admin: data?.is_admin === true
        }];
      }
    }

    const startIndex = (current - 1) * pageSize;
    const paginatedUsers = allUsers.slice(startIndex, startIndex + pageSize);

    return res.json({
      total: allUsers.length,
      data: paginatedUsers
    });
  } catch (error) {
    console.error('Error fetching users direct list:', error);
    return res.status(500).json({ error: 'Internal Server Error' });
  }
});

router.get('/peers', authenticateToken as any, async (req: AuthenticatedRequest, res: Response): Promise<any> => {
  if (!req.user) {
    return res.status(401).json({ error: 'Unauthorized' });
  }

  const current = parseInt(req.query.current as string) || 1;
  const pageSize = parseInt(req.query.pageSize as string) || 100;

  try {
    const db = admin.firestore();
    let peersSnapshot;

    if (req.user.isAdmin) {
      peersSnapshot = await db.collection('peers').get();
    } else {
      // Non-admins see all devices in their address books
      const abSnapshot = await db.collection('address_books')
        .where('owner', '==', req.user.uid)
        .get();

      const abGuids = abSnapshot.docs.map(doc => doc.id);
      
      if (abGuids.length === 0) {
        return res.json({ total: 0, data: [] });
      }

      peersSnapshot = await db.collection('peers')
        .where('ab', 'in', abGuids)
        .get();
    }

    const allPeers = peersSnapshot.docs.map(doc => doc.data());
    const startIndex = (current - 1) * pageSize;
    const paginatedPeers = allPeers.slice(startIndex, startIndex + pageSize);

    return res.json({
      total: allPeers.length,
      data: paginatedPeers
    });
  } catch (error) {
    console.error('Error fetching direct peers list:', error);
    return res.status(500).json({ error: 'Internal Server Error' });
  }
});

export default router;
