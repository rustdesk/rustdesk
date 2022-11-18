import { extendTheme } from "@chakra-ui/react"

import global from "./styles"
import colors from "./colors"

import FormLabel from "./components/FormLabel"
import Divider from "./components/Divider"
import Button from "./components/Button"

const overrides = {
  styles: {
    global
  },
  colors,
  shadows: {
    main: "0px 0px 10px rgba(0, 0, 0, 0.25)"
  },
  components: {
    FormLabel,
    Divider,
    Button
  }
}

export default extendTheme(overrides)
