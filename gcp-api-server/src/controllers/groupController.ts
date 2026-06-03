import { Response } from 'express';
import * as admin from 'firebase-admin';
import { AuthenticatedRequest } from '../middleware/auth';

/**
 * Fetch all accessible device groups for the user.
 * Return format complies with RustDesk pagination expected list layout.
 */
export const getAccessibleDeviceGroups = async (req: AuthenticatedRequest, res: Response): Promise<any> => {
  if (!req.user) {
    return res.status(401).json({ error: 'Unauthorized' });
  }

  const current = parseInt(req.query.current as string) || 1;
  const pageSize = parseInt(req.query.pageSize as string) || 100;

  try {
    const db = admin.firestore();
    let querySnapshot;

    if (req.user.isAdmin) {
      // Admins can see all device groups
      querySnapshot = await db.collection('device_groups').get();
    } else {
      // Standard users only see groups they created or have access to
      querySnapshot = await db.collection('device_groups')
        .where('accessible_users', 'array-contains', req.user.uid)
        .get();
    }

    const allGroups = querySnapshot.docs.map(doc => {
      const data = doc.data();
      return {
        name: data.name,
      };
    });

    // In-memory pagination
    const startIndex = (current - 1) * pageSize;
    const paginatedGroups = allGroups.slice(startIndex, startIndex + pageSize);

    return res.json({
      total: allGroups.length,
      data: paginatedGroups,
    });
  } catch (error) {
    console.error('Error fetching accessible device groups:', error);
    return res.status(500).json({ error: 'Internal Server Error' });
  }
};
