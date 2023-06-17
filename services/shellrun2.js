class Service extends ServiceBase {
    constructor(...args) {
        super(...args)
        
        this.supported = process.system.os !== "windows"
        this.config = process.getDotreplitConfig()

        if (this.supported) {
            this.running = false
            this.dead_ptys = []
            this.pty = new PtyProcess(this.id, process.env.SHELL || "sh", [], {
                "REPLIT_GIT_TOOLS_CHANNEL_FROM": this.id.toString()
            })
            this.pty.init(this.clients).then(_ => {
                console.debug("shell pty obtained:", this.pty.id)
            })
        } else {
            console.warn("Console isn't supported on windows")
        }
    }
    
	async recv(cmd, session) {
		if (cmd.input && this.supported) {
			await this.pty.write(cmd.input)
		} else if (cmd.runMain) {
            if (!this.supported) {
                await this.send(api.Command.create({state: api.State.Running}), 0)
                await this.send(api.Command.create({output:"[H[2J[3J\u001b[33m\u001b[39m Console is not supported for homeval on windows right now."}), 0)
                await this.send(api.Command.create({state: api.State.Stopped}), 0)
            } else if (!this.running) {
                // TODO: see how official impl deals with runMain while running
                this.dead_ptys.push(this.pty.id)
                await this.pty.destroy()
                this.running = true

                let cmd = "echo";
                let args = ["No run command set in your `.replit` file"]
                if (this.config.run) {
                    cmd = this.config.run.args[0]
                    args = this.config.run.args.slice(1)
                }

                this.pty = new PtyProcess(this.id, cmd, args, {
                    "REPLIT_GIT_TOOLS_CHANNEL_FROM": this.id.toString()
                })
                await this.send(api.Command.create({output:"[H[2J[3J" + `\u001b[33m\u001b[39m ${cmd} ${args.join(" ")}\u001b[K\r\n\u001b[0m`}), 0)
                await this.pty.init(this.clients)
                console.debug("Running command now", this.pty.id)
                await this.send(api.Command.create({state: api.State.Running}), 0)
            }
        } else if (cmd.clear && this.running) {
            await this.pty.destroy()
        }
	}

    async attach(session) {
        if (!this.supported) {
            await this.send(api.Command.create({output:"[H[2J[3J\u001b[33m\u001b[39m Console is not supported for homeval on windows right now."}), session)
            return;
        }

        await this.pty.add_session(session)
        await this.send(
            api.Command.create({
                state: this.running ? api.State.Running : api.State.Stopped
            }),
            session
        )
    }

    async detach(session) {
        if (this.supported) {
            await this.pty.remove_session(session)
        }
    }

    async process_dead(pty) {
        if (this.dead_ptys.includes(pty) || !this.supported) {return}
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
