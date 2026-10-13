import { mount } from 'svelte';
import App from './App.svelte';
import './styles.css';
import '@xterm/xterm/css/xterm.css';
import './blocks.css';

mount(App, { target: document.getElementById('app')! });
