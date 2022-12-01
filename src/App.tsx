import { BrowserRouter, Routes, Route } from "react-router-dom"
import { ChakraProvider } from "@chakra-ui/provider"
import theme from "./theme/index"

import Remotes from './pages/Remotes';

const App = () => {
    return (
      <BrowserRouter>
        <ChakraProvider theme={theme}>
          <Routes>
            <Route index element={<Remotes />} />
          </Routes>
        </ChakraProvider>
      </BrowserRouter>
    )
  }
  
  export default App
  
