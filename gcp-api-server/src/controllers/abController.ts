import { Response } from 'express';
import * as admin from 'firebase-admin';
import { AuthenticatedRequest } from '../middleware/auth';

/**
 * Get limits configuration for the Address Book.
 */
export const getAbSettings = async (req: AuthenticatedRequest, res: Response): Promise<any> => {
  return res.json({
    max_peer_one_ab: 1000,
  });
};

/**
 * Retrieve or dynamically initialize the personal address book for a user.
 */
export const getPersonalAb = async (req: AuthenticatedRequest, res: Response): Promise<any> => {
  if (!req.user) {
    return res.status(401).json({ error: 'Unauthorized' });
  }

  try {
    const db = admin.firestore();
    const abRef = db.collection('address_books');
    
    // Find if the user already has a personal address book
    const querySnapshot = await abRef
      .where('owner', '==', req.user.uid)
      .where('name', '==', 'Personal')
      .limit(1)
      .get();

    if (!querySnapshot.empty) {
      const doc = querySnapshot.docs[0];
      return res.json({ guid: doc.id });
    }

    // Provision new address book if none exists
    const newAbId = `ab_${Math.random().toString(36).substring(2, 15)}`;
    await abRef.doc(newAbId).set({
      guid: newAbId,
      name: 'Personal',
      owner: req.user.uid,
      note: 'Auto-provisioned personal address book',
      rule: 3, // Full Control
      tags: [],
      created_at: admin.firestore.FieldValue.serverTimestamp(),
    });

    console.log(`Initialized personal address book (${newAbId}) for user: ${req.user.email}`);
    return res.json({ guid: newAbId });
  } catch (error) {
    console.error('Error fetching personal address book:', error);
    return res.status(500).json({ error: 'Internal Server Error' });
  }
};

/**
 * Fetch a page of peers (devices) belonging to a specific address book.
 */
export const getPeers = async (req: AuthenticatedRequest, res: Response): Promise<any> => {
  const { ab, current, pageSize } = req.query;

  if (!ab) {
    return res.status(400).json({ error: 'Address Book GUID (ab) is required' });
  }

  const pageNum = parseInt(current as string) || 1;
  const sizeNum = parseInt(pageSize as string) || 100;

  try {
    const db = admin.firestore();
    
    // Query peers assigned to this address book
    const peersSnapshot = await db.collection('peers')
      .where('ab', '==', ab as string)
      .get();

    const allPeers = peersSnapshot.docs.map(doc => doc.data());
    
    // Perform in-memory pagination (highly robust for standard size books)
    const startIndex = (pageNum - 1) * sizeNum;
    const paginatedPeers = allPeers.slice(startIndex, startIndex + sizeNum);

    return res.json({
      total: allPeers.length,
      data: paginatedPeers,
    });
  } catch (error) {
    console.error('Error fetching peers:', error);
    return res.status(500).json({ error: 'Internal Server Error' });
  }
};

/**
 * Dynamic group registry helper.
 * If a device mentions a device_group_name, register it if it does not already exist.
 */
const registerDeviceGroupIfNeeded = async (groupName: string, ownerUid: string) => {
  if (!groupName || groupName.trim() === '') return;

  const db = admin.firestore();
  const groupSlug = groupName.toLowerCase().replace(/[^a-z0-9]+/g, '-');
  const groupRef = db.collection('device_groups').doc(groupSlug);
  const groupDoc = await groupRef.get();

  if (!groupDoc.exists) {
    console.log(`Dynamically creating new device group: "${groupName}" (slug: ${groupSlug})`);
    await groupRef.set({
      name: groupName,
      owner: ownerUid,
      accessible_users: [ownerUid],
      created_at: admin.firestore.FieldValue.serverTimestamp(),
    });
  }
};

/**
 * Add a peer to an address book.
 */
export const addPeer = async (req: AuthenticatedRequest, res: Response): Promise<any> => {
  if (!req.user) {
    return res.status(401).json({ error: 'Unauthorized' });
  }

  const { guid } = req.params; // Address book GUID
  const peerData = req.body;

  if (!peerData || !peerData.id) {
    return res.status(400).json({ error: 'Peer ID is required' });
  }

  try {
    const db = admin.firestore();
    
    // Authenticate writing permissions: check address book ownership
    const abDoc = await db.collection('address_books').doc(guid).get();
    if (!abDoc.exists) {
      return res.status(404).json({ error: 'Address Book not found' });
    }
    if (abDoc.data()?.owner !== req.user.uid && !req.user.isAdmin) {
      return res.status(403).json({ error: 'Access denied to this Address Book' });
    }

    // Dynamic Creation: Check for device_group_name and register if needed
    if (peerData.device_group_name) {
      await registerDeviceGroupIfNeeded(peerData.device_group_name, req.user.uid);
    }

    const peerPayload = {
      ...peerData,
      ab: guid,
      updated_at: admin.firestore.FieldValue.serverTimestamp(),
    };

    // Save/Overwrite peer doc in Firestore using client ID as Document ID
    await db.collection('peers').doc(`${guid}_${peerData.id}`).set(peerPayload);

    return res.json({ status: 'ok' });
  } catch (error) {
    console.error('Error adding peer:', error);
    return res.status(500).json({ error: 'Internal Server Error' });
  }
};

/**
 * Update peer info in an address book.
 */
export const updatePeer = async (req: AuthenticatedRequest, res: Response): Promise<any> => {
  if (!req.user) {
    return res.status(401).json({ error: 'Unauthorized' });
  }

  const { guid } = req.params;
  const peerData = req.body;

  if (!peerData || !peerData.id) {
    return res.status(400).json({ error: 'Peer ID is required' });
  }

  try {
    const db = admin.firestore();
    
    // Auth Check
    const abDoc = await db.collection('address_books').doc(guid).get();
    if (!abDoc.exists) {
      return res.status(404).json({ error: 'Address Book not found' });
    }
    if (abDoc.data()?.owner !== req.user.uid && !req.user.isAdmin) {
      return res.status(403).json({ error: 'Access denied to this Address Book' });
    }

    // Dynamic Creation: Check for device_group_name and register if needed
    if (peerData.device_group_name) {
      await registerDeviceGroupIfNeeded(peerData.device_group_name, req.user.uid);
    }

    const docId = `${guid}_${peerData.id}`;
    const peerRef = db.collection('peers').doc(docId);
    const peerDoc = await peerRef.get();

    if (!peerDoc.exists) {
      return res.status(404).json({ error: 'Peer not found in this Address Book' });
    }

    // Merge updates
    await peerRef.update({
      ...peerData,
      updated_at: admin.firestore.FieldValue.serverTimestamp(),
    });

    return res.json({ status: 'ok' });
  } catch (error) {
    console.error('Error updating peer:', error);
    return res.status(500).json({ error: 'Internal Server Error' });
  }
};

/**
 * Delete one or multiple peers from an address book.
 */
export const deletePeers = async (req: AuthenticatedRequest, res: Response): Promise<any> => {
  if (!req.user) {
    return res.status(401).json({ error: 'Unauthorized' });
  }

  const { guid } = req.params;
  const peerIds = req.body; // Expects array of peer IDs, e.g. ["123456789"]

  if (!Array.isArray(peerIds) || peerIds.length === 0) {
    return res.status(400).json({ error: 'Array of Peer IDs is required' });
  }

  try {
    const db = admin.firestore();
    
    // Auth Check
    const abDoc = await db.collection('address_books').doc(guid).get();
    if (!abDoc.exists) {
      return res.status(404).json({ error: 'Address Book not found' });
    }
    if (abDoc.data()?.owner !== req.user.uid && !req.user.isAdmin) {
      return res.status(403).json({ error: 'Access denied to this Address Book' });
    }

    const batch = db.batch();
    for (const id of peerIds) {
      const docId = `${guid}_${id}`;
      const peerRef = db.collection('peers').doc(docId);
      batch.delete(peerRef);
    }

    await batch.commit();
    console.log(`Deleted peers [${peerIds.join(', ')}] from Address Book ${guid}`);
    return res.json({ status: 'ok' });
  } catch (error) {
    console.error('Error deleting peers:', error);
    return res.status(500).json({ error: 'Internal Server Error' });
  }
};

/**
 * Get all tags for an address book.
 */
export const getTags = async (req: AuthenticatedRequest, res: Response): Promise<any> => {
  const { guid } = req.params;

  try {
    const db = admin.firestore();
    const abDoc = await db.collection('address_books').doc(guid).get();
    
    if (!abDoc.exists) {
      return res.status(404).json({ error: 'Address Book not found' });
    }

    const tags = abDoc.data()?.tags || [];
    return res.json(tags);
  } catch (error) {
    console.error('Error fetching tags:', error);
    return res.status(500).json({ error: 'Internal Server Error' });
  }
};

/**
 * Add a new tag to the address book config.
 */
export const addTag = async (req: AuthenticatedRequest, res: Response): Promise<any> => {
  const { guid } = req.params;
  const { name, color } = req.body;

  if (!name || color === undefined) {
    return res.status(400).json({ error: 'Tag name and color value are required' });
  }

  try {
    const db = admin.firestore();
    const abRef = db.collection('address_books').doc(guid);
    const abDoc = await abRef.get();

    if (!abDoc.exists) {
      return res.status(404).json({ error: 'Address Book not found' });
    }

    const tags = abDoc.data()?.tags || [];
    
    // Check if tag name already exists
    if (tags.some((t: any) => t.name === name)) {
      return res.status(400).json({ error: `Tag "${name}" already exists` });
    }

    tags.push({ name, color });
    await abRef.update({ tags });

    return res.json({ status: 'ok' });
  } catch (error) {
    console.error('Error adding tag:', error);
    return res.status(500).json({ error: 'Internal Server Error' });
  }
};

/**
 * Rename a tag in the address book config.
 */
export const renameTag = async (req: AuthenticatedRequest, res: Response): Promise<any> => {
  const { guid } = req.params;
  const { old: oldName, new: newName } = req.body;

  if (!oldName || !newName) {
    return res.status(400).json({ error: 'Old and new tag names are required' });
  }

  try {
    const db = admin.firestore();
    const abRef = db.collection('address_books').doc(guid);
    const abDoc = await abRef.get();

    if (!abDoc.exists) {
      return res.status(404).json({ error: 'Address Book not found' });
    }

    let tags = abDoc.data()?.tags || [];
    let updated = false;

    tags = tags.map((t: any) => {
      if (t.name === oldName) {
        updated = true;
        return { ...t, name: newName };
      }
      return t;
    });

    if (!updated) {
      return res.status(404).json({ error: `Tag "${oldName}" not found` });
    }

    await abRef.update({ tags });

    // Also update any references to this tag on individual peers in this address book
    const peersSnapshot = await db.collection('peers')
      .where('ab', '==', guid)
      .where('tags', 'array-contains', oldName)
      .get();

    if (!peersSnapshot.empty) {
      const batch = db.batch();
      peersSnapshot.forEach(doc => {
        const peerTags = doc.data().tags || [];
        const newTags = peerTags.map((t: string) => t === oldName ? newName : t);
        batch.update(doc.ref, { tags: newTags });
      });
      await batch.commit();
    }

    return res.json({ status: 'ok' });
  } catch (error) {
    console.error('Error renaming tag:', error);
    return res.status(500).json({ error: 'Internal Server Error' });
  }
};

/**
 * Update tag color.
 */
export const updateTag = async (req: AuthenticatedRequest, res: Response): Promise<any> => {
  const { guid } = req.params;
  const { name, color } = req.body;

  if (!name || color === undefined) {
    return res.status(400).json({ error: 'Tag name and color are required' });
  }

  try {
    const db = admin.firestore();
    const abRef = db.collection('address_books').doc(guid);
    const abDoc = await abRef.get();

    if (!abDoc.exists) {
      return res.status(404).json({ error: 'Address Book not found' });
    }

    let tags = abDoc.data()?.tags || [];
    let updated = false;

    tags = tags.map((t: any) => {
      if (t.name === name) {
        updated = true;
        return { name, color };
      }
      return t;
    });

    if (!updated) {
      return res.status(404).json({ error: `Tag "${name}" not found` });
    }

    await abRef.update({ tags });
    return res.json({ status: 'ok' });
  } catch (error) {
    console.error('Error updating tag:', error);
    return res.status(500).json({ error: 'Internal Server Error' });
  }
};

/**
 * Delete a tag from the address book and clear it from peers.
 */
export const deleteTag = async (req: AuthenticatedRequest, res: Response): Promise<any> => {
  const { guid } = req.params;
  const tagList = req.body; // Expects an array containing tag name to delete, e.g. ["tag_to_delete"]

  if (!Array.isArray(tagList) || tagList.length === 0) {
    return res.status(400).json({ error: 'Tag name list array is required' });
  }

  const tagName = tagList[0];

  try {
    const db = admin.firestore();
    const abRef = db.collection('address_books').doc(guid);
    const abDoc = await abRef.get();

    if (!abDoc.exists) {
      return res.status(404).json({ error: 'Address Book not found' });
    }

    let tags = abDoc.data()?.tags || [];
    tags = tags.filter((t: any) => t.name !== tagName);
    await abRef.update({ tags });

    // Also remove the tag from any peer that has it
    const peersSnapshot = await db.collection('peers')
      .where('ab', '==', guid)
      .where('tags', 'array-contains', tagName)
      .get();

    if (!peersSnapshot.empty) {
      const batch = db.batch();
      peersSnapshot.forEach(doc => {
        const peerTags = doc.data().tags || [];
        const newTags = peerTags.filter((t: string) => t !== tagName);
        batch.update(doc.ref, { tags: newTags });
      });
      await batch.commit();
    }

    return res.json({ status: 'ok' });
  } catch (error) {
    console.error('Error deleting tag:', error);
    return res.status(500).json({ error: 'Internal Server Error' });
  }
};
