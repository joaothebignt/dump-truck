import { createRouter, createWebHashHistory } from "vue-router"

import Home from "../pages/Home.vue"
import NewConnection from "../pages/NewConnection.vue"
import Database from "../pages/Database.vue"

const routes = [
  {
    path: "/",
    component: Home
  },
  {
    path: "/connection/new",
    component: NewConnection
  },
  {
    path: "/connection/edit/:index",
    component: NewConnection
  },
  {
    path: "/database",
    component: Database
  }
]

export default createRouter({
  history: createWebHashHistory(),
  routes
})