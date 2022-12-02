import React from 'react'
import { invoke } from '@tauri-apps/api'
import { emit, listen } from '@tauri-apps/api/event'

const Remote = () => {
    invoke('reconnect')
        .then(() => {
            listen('native-remote', (e) => {
                console.log(e)
            })
        })

    // TEST event
    listen('click', (event) => {
        // console.log(event)
    })

    // emits the `click` event with the object payload
    emit('click', {
        theMessage: 'Tauri is awesome!',
    })
    
    return (
        <div>
            Hi
        </div>
    )
}

export default Remote
