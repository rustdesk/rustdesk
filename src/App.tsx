import { BrowserRouter, Routes, Route, redirect } from "react-router-dom"
import { ChakraProvider } from "@chakra-ui/provider"
import theme from "./theme/index"

import Remotes from './pages/Remotes'
import Remote from './pages/Remote'

import { window as TWindow } from '@tauri-apps/api'

const l = '/' + TWindow.getCurrent().label
// FIXME use router API?
if (window.location.pathname !== l) {
  window.location.pathname = l
}

const App = () => {
    return (
      <BrowserRouter>
        <ChakraProvider theme={theme}>
          <Routes>
            <Route path='/main' element={<Remotes />} />
            <Route path='/remote' element={<Remote />} />
          </Routes>
        </ChakraProvider>
      </BrowserRouter>
    )
  }
  
  export default App
  
