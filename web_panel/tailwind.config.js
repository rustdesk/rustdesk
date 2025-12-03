/** @type {import('tailwindcss').Config} */
module.exports = {
  content: [
    "./server.py",
    "./templates/**/*.html"
  ],
  darkMode: 'class',
  theme: {
    extend: {
      colors: {
        primary: {
          DEFAULT: '#0d6efd',
          hover: '#0b5ed7',
          light: 'rgba(13, 110, 253, 0.1)'
        },
        surface: {
          light: '#ffffff',
          dark: '#212529'
        },
        body: {
          light: '#f8f9fa',
          dark: '#1a1d21'
        }
      },
      width: {
        'sidebar': '250px'
      },
      margin: {
        'sidebar': '250px'
      },
      fontFamily: {
        mono: ['Consolas', 'Monaco', 'monospace']
      }
    }
  },
  plugins: [require("daisyui")]
}


