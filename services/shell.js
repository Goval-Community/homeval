class Service extends ServiceBase {
    constructor(...args) {
        super(...args)
        this.supported = process.system.os !== "windows"

        if (this.supported) {
            this.pty = new PtyProcess(this.id, process.env.SHELL || "sh", [], {
                "REPLIT_GIT_TOOLS_CHANNEL_FROM": this.id.toString()
            })
    
            this.pty.init(this.clients).then(_ => {
                console.debug("shell pty obtained:", this.pty.id)
            })
            this.dead_ptys = []
        } else {
            console.warn("Shell isn't supported on windows")
        }
        
    }

    async process_dead(pty) {
        if (this.dead_ptys.includes(pty) || !this.supported) {return}
        this.dead_ptys.push(pty)

        this.pty = new PtyProcess(this.id, process.env.SHELL || "sh", [], {
            "REPLIT_GIT_TOOLS_CHANNEL_FROM": this.id.toString()
        });
        
        await this.pty.init(this.clients)
        console.debug("shell pty obtained:", this.pty.id)
    }
    
	async recv(cmd, session) {
		if (cmd.input && this.supported) {
			await this.pty.write(cmd.input)
		}
	}

    async attach(session) {
        if (!this.supported) {
            await this.send(api.Command.create({output:"[H[2J[3J\u001b[33m\u001b[39m Shell is not supported for homeval on windows right now."}), session)
            return
        }

        await this.pty.add_session(session)
        
    }

    async detach(session) {
        if (this.supported) {
            await this.pty.remove_session(session)
        }
    }
}

const service = new Service(
	serviceInfo.id,
	serviceInfo.service,
	serviceInfo.name,
);
await service.start()
