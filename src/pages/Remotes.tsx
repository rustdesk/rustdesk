import React, { useState } from 'react'
import { Input, Box, Heading, Button } from '@chakra-ui/react'

import { invoke } from "@tauri-apps/api"

const Remotes = () => {
    const [remoteId, setRemoteId] = useState<string>('')

    const createNewConnect = (id: string, type: string) => {
        const _id = id.replace(/\s/g, "")
        if (!_id) return
        // TODO where I can find my_id?
        // if (_id === my_id)
    
        invoke('set_remote_id', { id: _id }).then(console.log)
        invoke('new_remote', { id: _id, remoteType: type }).then(console.log)
    }

    return (
        <Box>
            <Heading>Control remote desktop</Heading>
            <Input
                value={remoteId}
                onChange={e => {
                    setRemoteId(e.target.value)
                }} />
            <Button onClick={() => createNewConnect(remoteId, 'connect')}>Connect</Button>
        </Box>
    )
}

export default Remotes
