import {
  Button,
  FormControl,
  FormErrorMessage,
  FormHelperText,
  FormLabel,
  Input,
} from '@chakra-ui/react';

import React, { useState, useEffect } from "react";


import { listen } from '@tauri-apps/api/event'
import { addConnection } from "../cm";
import { invoke } from "@tauri-apps/api";


// TODO: removeConnection
// TODO: new_message

function Form() {
  const [firstname, setFirstname] = useState('');
  const [lastname, setLastname] = useState('');

  const handleFirstname = e => setFirstname(e.target.value);
  const handleLastname = e => setLastname(e.target.value);

  const handleClick = ()=>{
    alert(`Hello ${firstname} ${lastname}`)
    setFirstname("")
    setLastname("")
    console.log("test_tauri")
  }

  useEffect(() => {
    console.log("useEffect")
    if (listen) {
      // TODO: Test with tauri windows dev tools
      listen('addConnection', (event) => {
        console.log(event.payload);
        addConnection(event.payload.id, event.payload.is_file_transfer, event.payload.port_forward, event.payload.peer_id, event.payload.name, event.payload.authorized, event.payload.keyboard, event.payload.clipboard, event.payload.audio, event.payload.file, event.payload.restart, event.payload.recording);
        // event.event is the event name (useful if you want to use a single callback fn for multiple event types)
        // event.payload is the payload object
      })
    }
  });

  async function search() {
    // Learn more about Tauri commands at https://tauri.app/v1/guides/features/command

    const temp = (await invoke("test_tauri")
    .then(()=>console.log("test_tauri")));
  }


//   const isError = input === '';

  return (
    <FormControl
    //   isInvalid={isError}
      w="50%"
      mx="20%"
      mt="20px"
      pt={"5px"}
      border={'2px solid #59C8FF'}
      borderRadius={'10px'}
    >
      <FormLabel htmlFor="Firstname">Firstname</FormLabel>
      <Input
        id="firstName"
        type="text"
        value={firstname}
        onChange={handleFirstname}
        mb="20px"
      />

      <FormLabel htmlFor="Lastname">Lastname</FormLabel>
      <Input
        id="lastName"
        type="text"
        value={lastname}
        onChange={handleLastname}
      />
      {/* {!isError ? (
        <FormHelperText>
          Enter the email you'd like to receive the newsletter on.
        </FormHelperText>
      ) : (
        <FormErrorMessage>Email is required.</FormErrorMessage>
      )} */}
      <Button colorScheme="blue" variant="solid" mt={"10px"} onClick={handleClick} mx={"40%"}>
        GREET ME
      </Button>
    </FormControl>
  );
}

export default Form;
