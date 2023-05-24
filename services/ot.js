class Service extends ServiceBase {
	constructor(...args) {
		super(...args);
		this.version = 1
		this.contents = ""
		this.path = null
		this.cursors = {}
		this.history = []
		this.session_info = {}
	}

	async recv(cmd, session) {
		if (!this.path && !cmd.otLinkFile) {
			console.error("Command sent before otLinkFile", cmd)
			return
		}

		if (cmd.otLinkFile) {
			const path = cmd.otLinkFile.file.path;
			const content = await fs.readFile(path);
			this.path = path
			this.contents = await fs.readFileString(path);

			this.history.push({
				spookyVersion: this.version,
				op: [{insert: this.contents}],
				crc32: CRC32.str(this.contents),
				committed: {
					seconds: (Date.now()/ 1000n).toString(),
					nanos: 0
				},
				version: this.version,
				author: api.OTPacket.Author.USER,
				// use https://replit.com/@homeval for initial insert
				userId: 23352071
			})

			return api.Command.create({otLinkFileResponse:{version:this.version, linkedFile:{path, content}}})
		} else if (cmd.ot) {
			let cursor = 0
			let contents = [...this.contents]

			for (const op of cmd.ot.op) {
				if (op.skip) {
					const skip = op.skip
					if (skip + cursor > contents.length) {
						throw new Error("Invalid skip past bounds")
					}

					cursor += skip
				}
				if (op.insert) {
					const insert = op.insert
					contents = [...contents.slice(0, cursor), ...insert, ...contents.slice(cursor)]
					cursor += insert.length
				}
				if (op.delete) {
					const del = op.delete
					if (del + cursor > contents.length) {
						throw new Error("Invalid delete past bounds")
					}

					contents = [...contents.slice(0,cursor), ...contents.slice(cursor + del)]
				}
			}

			const final_contents = contents.join("")

			this.version += 1
			this.contents = final_contents

			const inner_packet = {
				spookyVersion: this.version,
				op: cmd.ot.op,
				crc32: CRC32.str(final_contents),
				committed: {
					seconds: (Date.now()/ 1000n).toString(),
					nanos: 0
				},
				version: this.version,
				author: cmd.ot.author,
				// use https://replit.com/@ghostwriterai if ghostwriter wrote it (for history)
				userId: cmd.ot.author !== api.OTPacket.Author.GHOSTWRITER ? this.session_info[session].id : 22261053
			}

			this.history.push(inner_packet)

			const msg = api.Command.create({
				ot: inner_packet,
				ref: cmd.ref
			})

			await this.send(msg,session)

			for (const arr_session of this.clients) {
				if (arr_session === session) {continue}
				await this.send(msg, arr_session)
			}

			await fs.writeFileString(this.path, final_contents)
		} else if (cmd.otNewCursor) {
			const cursor = cmd.otNewCursor
			this.cursors[cursor.id] = cursor

			const msg = api.Command.create({ otNewCursor: cursor })

			await this.send(msg, -session)
		} else if (cmd.otDeleteCursor) {
			delete this.cursors[cmd.otDeleteCursor.id]

			const msg = api.Command.create({ otDeleteCursor: cmd.otDeleteCursor })

			await this.send(msg, -session)
		} else if (cmd.otFetchRequest) {
			return api.Command.create({
				otFetchResponse: {
					packets: this.history.slice(
						cmd.otFetchRequest.versionFrom - 1,
						cmd.otFetchRequest.versionTo + 1
					)
				}
			})
		} else {
			console.warn("Unknown ot command", cmd)
		}
	}

	async attach(session) {
		this.session_info[session] = process.getUserInfo(session)
		
		if (!this.path) {
			await this.send(api.Command.create({otstatus: {}}), session)
			return
		}

		

		await this.send(api.Command.create({
			otstatus:{
				contents:this.contents, 
				version: this.version, 
				linkedFile: {path:this.path},
				cursors: Object.values(this.cursors)
			}
		}), session)
		
	}
}

const service = new Service(
	serviceInfo.id,
	serviceInfo.service,
	serviceInfo.name,
);
await service.start()
