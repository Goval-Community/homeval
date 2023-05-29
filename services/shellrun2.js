class Service extends ServiceBase {
    constructor(...args) {
        super(...args)
        this.running = false
        this.dead_ptys = []
        this.pty = new PtyProcess(this.id, process.env.SHELL || "sh")
        this.pty.init(this.clients).then(_ => {
            console.debug("shell pty obtained:", this.pty.id)
        })
    }
    
	async recv(cmd, session) {
		if (cmd.input) {
			await this.pty.write(cmd.input)
		} else if (cmd.runMain && !this.running) {
            // TODO: see how official impl deals with runMain while running
            this.dead_ptys.push(this.pty.id)
            await this.pty.destroy()
            this.running = true

            this.pty = new PtyProcess(this.id, "./target/release/homeval", ["127.0.0.1:8081"])
            await this.send(api.Command.create({output:"[H[2J[3J" + "\u001b[33m\u001b[39m ./target/release/homeval 127.0.0.1:8081\u001b[K\r\n\u001b[0m"}), 0)
            await this.pty.init(this.clients)
            console.debug("Running command now", this.pty.id)
            await this.send(api.Command.create({state: api.State.Running}), 0)
        } else if (cmd.clear && this.running) {
            await this.pty.destroy()
        }
	}

    async attach(session) {
        await this.pty.add_session(session)
        await this.send(
            api.Command.create({
                state: this.running ? api.State.Running : api.State.Stopped
            }),
            session
        )
    }

    async detach(session) {
        await this.pty.remove_session(session)
    }

    async pty_died(pty) {
        if (this.dead_ptys.includes(pty)) {return}
        this.dead_ptys.push(pty)
        // 
        if (this.running) {
            this.running = false
            await this.send(api.Command.create({state: api.State.Stopped}), 0)
        } else {
        }
        
        try {await this.pty.destroy()} catch(err) {}

        this.pty = new PtyProcess(this.id, process.env.SHELL || "sh")
        await this.pty.init(this.clients)
        console.debug("shell pty obtained:", this.pty.id)
    }
}

const service = new Service(
	serviceInfo.id,
	serviceInfo.service,
	serviceInfo.name,
);
await service.start()
