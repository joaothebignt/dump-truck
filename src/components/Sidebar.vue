<script setup>

import { computed, onMounted } from "vue"
import { useRouter } from "vue-router"
import { invoke } from "@tauri-apps/api/core"

import { useConnectionsStore } from "../stores/connections"
import { useDatabaseStore } from "../stores/database"

const store = useConnectionsStore()
const dbStore = useDatabaseStore()
const router = useRouter()

const hasConnection = computed(() => !!dbStore.connection)

onMounted(()=>{
  store.load()
})

function newConnection(){
  router.push("/connection/new")
}

async function connect(connection){

  if (!connection || !connection.host) {

    alert("Conexão inválida: host não definido. Edite ou crie uma nova conexão.")
    return

  }

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
    alert("Erro ao conectar ao banco de dados.\n\nDetalhes: " + e)

  }

}

function disconnect(){

  dbStore.clearConnection()
  router.push("/")

}

</script>

<template>

<div class="sidebar-container">

  <div class="section">

    <div class="section-title">
      Connections
    </div>

    <div
      class="connection"
      v-for="(c, index) in store.favorites"
      :key="c.name"
    >

      <span
        class="connection-name"
        @click="connect(c)"
      >
        {{c.name}}
      </span>

      <button
        class="delete-btn"
        @click.stop="store.remove(index)"
        title="Excluir conexão"
      >
        ×
      </button>

    </div>

    <button
      class="new-btn"
      @click="newConnection"
    >
      + Nova conexão
    </button>

    <button
      v-if="hasConnection"
      class="disconnect-btn"
      @click="disconnect"
    >
      Fechar conexão atual
    </button>

  </div>

</div>

</template>

<style scoped>

.sidebar-container{
  padding:10px;
}

.section-title{
  font-size:12px;
  text-transform:uppercase;
  margin-bottom:8px;
  color:#aaa;
}

.connection{
  padding:6px 8px;
  border-radius:4px;
  cursor:pointer;
  display:flex;
  align-items:center;
  justify-content:space-between;
}

.connection:hover{
  background:#333;
}

.delete-btn{
  background:transparent;
  border:none;
  color:#888;
  cursor:pointer;
  font-size:14px;
  padding:0 4px;
}

.delete-btn:hover{
  color:#f88;
}

.new-btn{
  margin-top:10px;
  width:100%;
  background:#3a7afe;
  border:none;
  color:white;
  padding:6px;
  border-radius:4px;
  cursor:pointer;
}

.disconnect-btn{
  margin-top:6px;
  width:100%;
  background:#555;
  border:none;
  color:white;
  padding:6px;
  border-radius:4px;
  cursor:pointer;
}

</style>