<script setup>

import { onMounted } from "vue"
import { useRouter } from "vue-router"
import { invoke } from "@tauri-apps/api/core"

import { useConnectionsStore } from "../stores/connections"
import { useDatabaseStore } from "../stores/database"

const router = useRouter()
const store = useConnectionsStore()
const dbStore = useDatabaseStore()

onMounted(() => {

  store.load()

})

function newConnection() {

  router.push("/connection/new")

}

async function connect(connection) {

  try{

    const conn = await invoke("connect_db",{
      host: connection.host,
      port: Number(connection.port),
      user: connection.username,
      pass: connection.password
    })

    dbStore.setConnection(conn)
    dbStore.selectedDatabase = null
    dbStore.tables = []

    router.push("/database")

  }catch(e){

    console.error(e)

  }

}

</script>

<template>

<h1>DB Dumper</h1>

<button @click="newConnection">
Nova conexão
</button>

<h2>Favoritas</h2>

<div
v-for="c in store.favorites"
:key="c.name"
>

{{ c.name }} - {{ c.host }}

<button @click="connect(c)">
Conectar
</button>

</div>

</template>