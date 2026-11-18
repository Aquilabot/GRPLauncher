// GRP launcher: signs a player in to our account service, then starts the unmodified Ghost Recon Phantoms client
// against our servers and answers what the game asks its launcher over \\.\pipe\Ghost_Recon_Phantoms.
// No game file is changed and nothing of the game is shipped with this program.
//
// It also keeps itself and the game up to date: at start it asks the server for the current launcher and replaces
// itself when there is a newer one; after signing in it compares the game folder with the server's file list
// (manifest.txt of the vault) and fetches what is missing or different, so every player runs the same client.
//
//   GRPLauncher.exe                                   the window
//   GRPLauncher.exe --user N --password P [--invite C] [--gamedir <folder>] [--nosync]   no window (tests)
//
// Settings next to the program, launcher.ini: server, accounthost, accountport, port, accesskey (see Config).
// What the player typed last (name and game folder, never the password): %APPDATA%\GRPLauncher\settings.txt
using System;
using System.Collections.Generic;
using System.Diagnostics;
using System.Drawing;
using System.IO;
using System.Linq;
using System.Net;
using System.Runtime.InteropServices;
using System.Security.Cryptography;
using System.Text;
using System.Threading;
using System.Windows.Forms;

static class Config
{
    public const int Version = 2;               // of this launcher; the server tells when there is a newer one
    public static string Server = "56.127.0.17";
    public static string AccountHost = "";      // account service and downloads; empty = the same as Server
    public static int AccountPort = 21080;      // TCP, account service
    public static int Port = 21031;             // UDP, lobby
    public static string AccessKey = "8dtRv2oj";
    public static string Language = "english";

    public static string DataDir { get { return Path.Combine(Environment.GetFolderPath(Environment.SpecialFolder.ApplicationData), "GRPLauncher"); } }
    public const string GameExe = "GhostReconPhantoms.exe";
    // Where the launcher installs the game, and where Phoenix's launcher left its copy (files that are identical
    // are copied from there instead of downloaded; that folder is never written to).
    public static string DefaultGameDir { get { return Path.Combine(Environment.GetFolderPath(Environment.SpecialFolder.LocalApplicationData), "GRP", "Ghost Recon Phantoms"); } }
    public static string PhoenixGameDir { get { return Path.Combine(Environment.GetFolderPath(Environment.SpecialFolder.LocalApplicationData), "Phoenix Network", "Ghost Recon Phantoms"); } }
    public static string BaseUrl { get { return "http://" + (AccountHost != "" ? AccountHost : Server) + ":" + AccountPort; } }

    public static Dictionary<string, string> ReadPairs(string file)
    {
        var d = new Dictionary<string, string>(StringComparer.OrdinalIgnoreCase);
        if (!File.Exists(file)) return d;
        foreach (string line in File.ReadAllLines(file))
        {
            int eq = line.IndexOf('=');
            if (eq > 0 && !line.TrimStart().StartsWith("#")) d[line.Substring(0, eq).Trim()] = line.Substring(eq + 1).Trim();
        }
        return d;
    }

    public static void Load()
    {
        var d = ReadPairs(Path.Combine(AppDomain.CurrentDomain.BaseDirectory, "launcher.ini"));
        string v; int n;
        if (d.TryGetValue("server", out v) && v != "") Server = v;
        if (d.TryGetValue("accounthost", out v)) AccountHost = v;
        if (d.TryGetValue("accountport", out v) && int.TryParse(v, out n)) AccountPort = n;
        if (d.TryGetValue("port", out v) && int.TryParse(v, out n)) Port = n;
        if (d.TryGetValue("accesskey", out v) && v != "") AccessKey = v;
        if (d.TryGetValue("language", out v) && v != "") Language = v;
    }
}

static class Account
{
    // What goes to the server instead of the password: PBKDF2-HMAC-SHA1(password, "grp2016:" + lowercase name),
    // 50000 rounds, 32 bytes, in hex. The server stores it salted and hashed again.
    public static string Secret(string name, string password)
    {
        using (var k = new Rfc2898DeriveBytes(Encoding.UTF8.GetBytes(password), Encoding.UTF8.GetBytes("grp2016:" + name.ToLowerInvariant()), 50000))
            return string.Concat(k.GetBytes(32).Select(b => b.ToString("x2")));
    }

    // One line back from the account service: "OK ..." or "ERR <reason>"; a network failure comes back as "NET <text>".
    public static string Post(string path, string form)
    {
        try
        {
            var req = (HttpWebRequest)WebRequest.Create(Config.BaseUrl + path);
            req.Method = "POST"; req.ContentType = "application/x-www-form-urlencoded"; req.Timeout = 10000; req.Proxy = null;
            byte[] body = Encoding.ASCII.GetBytes(form);
            req.ContentLength = body.Length;
            using (var s = req.GetRequestStream()) s.Write(body, 0, body.Length);
            using (var resp = (HttpWebResponse)req.GetResponse())
            using (var r = new StreamReader(resp.GetResponseStream())) return r.ReadToEnd().Trim();
        }
        catch (WebException ex)
        {
            if (ex.Response != null)
                using (var r = new StreamReader(ex.Response.GetResponseStream())) return r.ReadToEnd().Trim();
            return "NET " + ex.Message;
        }
        catch (Exception ex) { return "NET " + ex.Message; }
    }

    public static string Register(string invite, string name, string password)
    {
        return Post("/register", "invite=" + Uri.EscapeDataString(invite) + "&name=" + Uri.EscapeDataString(name) + "&secret=" + Secret(name, password));
    }

    public static string Login(string name, string password)
    {
        return Post("/login", "name=" + Uri.EscapeDataString(name) + "&secret=" + Secret(name, password) + "&v=" + Config.Version);
    }

    public static string Explain(string answer)
    {
        switch (answer)
        {
            case "ERR invite": return "El código de invitación no es válido o ya se usó.";
            case "ERR name": return "El nickname debe tener de 3 a 16 caracteres: letras, números, guion y guion bajo.";
            case "ERR taken": return "Ese nickname ya está en uso.";
            case "ERR login": return "Nickname o contraseña incorrectos.";
            case "ERR slow": return "Demasiados intentos. Espera un minuto.";
            case "ERR update": return "Este launcher es antiguo. Ciérralo y vuelve a abrirlo para que se actualice.";
            case "ERR secret": case "ERR size": case "ERR path": case "ERR template": return "El servidor rechazó la solicitud (" + answer + ").";
        }
        if (answer.StartsWith("NET ")) return "No se pudo conectar con el servidor: " + answer.Substring(4);
        return "Respuesta inesperada del servidor: " + answer;
    }
}

// Keeps the launcher and the game folder equal to what the server publishes.
static class Updater
{
    static HttpWebRequest Request(string url, long from = 0)
    {
        var req = (HttpWebRequest)WebRequest.Create(url);
        req.Proxy = null; req.Timeout = 15000; req.ReadWriteTimeout = 30000;
        if (from > 0) req.AddRange(from);
        return req;
    }

    public static string GetText(string url)
    {
        using (var resp = (HttpWebResponse)Request(url).GetResponse())
        using (var r = new StreamReader(resp.GetResponseStream())) return r.ReadToEnd();
    }

    public static string Sha256(string file)
    {
        using (var sha = SHA256.Create())
        using (var f = new FileStream(file, FileMode.Open, FileAccess.Read, FileShare.Read, 1 << 20))
            return string.Concat(sha.ComputeHash(f).Select(b => b.ToString("x2")));
    }

    // Downloads to <file>.part, continuing a partial one, and moves it into place when the hash is right.
    public static void Download(string url, string file, long size, string sha, Action<long> progress)
    {
        string part = file + ".part";
        Directory.CreateDirectory(Path.GetDirectoryName(file));
        Exception last = null;
        for (int attempt = 0; attempt < 6; attempt++)
        {
            try
            {
                long have = File.Exists(part) ? new FileInfo(part).Length : 0;
                if (have > size) { File.Delete(part); have = 0; }
                if (have < size)
                {
                    using (var resp = (HttpWebResponse)Request(url, have).GetResponse())
                    {
                        if (have > 0 && resp.StatusCode != HttpStatusCode.PartialContent) { have = 0; }
                        using (var net = resp.GetResponseStream())
                        using (var f = new FileStream(part, have > 0 ? FileMode.Append : FileMode.Create, FileAccess.Write))
                        {
                            var buf = new byte[1 << 16];
                            int n;
                            while ((n = net.Read(buf, 0, buf.Length)) > 0) { f.Write(buf, 0, n); have += n; progress(n); }
                        }
                    }
                }
                if (new FileInfo(part).Length == size && Sha256(part) == sha)
                {
                    if (File.Exists(file)) File.Delete(file);
                    File.Move(part, file);
                    return;
                }
                File.Delete(part);   // wrong content: start over
                last = new IOException("el archivo descargado no coincide");
            }
            catch (Exception ex) { last = ex; Thread.Sleep(2000); }
        }
        throw new IOException("No se pudo descargar " + Path.GetFileName(file) + ": " + (last == null ? "" : last.Message));
    }

    // Replaces this program when the server has a newer one, and starts it again. /launcher/version.txt holds
    // "<number> <sha256 of GRPLauncher.exe>". Returns true when the new one was started (the caller exits).
    public static bool SelfUpdate(string[] args)
    {
        string exe = System.Reflection.Assembly.GetExecutingAssembly().Location;
        try { if (File.Exists(exe + ".old")) File.Delete(exe + ".old"); } catch { }
        try
        {
            string[] v = GetText(Config.BaseUrl + "/launcher/version.txt").Trim().Split(' ');
            int n;
            if (v.Length < 2 || !int.TryParse(v[0], out n) || n <= Config.Version) return false;
            Program.Log("launcher " + n + " available (this is " + Config.Version + ")");
            string fresh = exe + ".new";
            if (File.Exists(fresh + ".part")) File.Delete(fresh + ".part");
            using (var resp = (HttpWebResponse)Request(Config.BaseUrl + "/launcher/GRPLauncher.exe").GetResponse())
            using (var net = resp.GetResponseStream())
            using (var f = new FileStream(fresh, FileMode.Create, FileAccess.Write)) net.CopyTo(f);
            if (Sha256(fresh) != v[1].ToLowerInvariant()) { File.Delete(fresh); Program.Log("new launcher: wrong hash, kept the current one"); return false; }
            File.Move(exe, exe + ".old");   // a running program can be renamed, not overwritten
            File.Move(fresh, exe);
            Process.Start(new ProcessStartInfo(exe, string.Join(" ", args.Select(a => a.Contains(" ") ? "\"" + a + "\"" : a))) { UseShellExecute = false });
            return true;
        }
        catch (Exception ex) { Program.Log("launcher update check failed: " + ex.Message); return false; }
    }

    class Entry { public bool mustMatch; public string sha; public long size; public string path; }

    // Makes the game folder equal to the vault's file list. progress(text, fraction 0..1).
    // Returns null when the folder is right, or the message for the player.
    public static string SyncGame(string vaultKey, string gameDir, Action<string, double> progress)
    {
        string baseUrl = Config.BaseUrl + "/files/" + vaultKey + "/";
        var entries = new List<Entry>();
        string vault = "";
        try
        {
            foreach (string line in GetText(baseUrl + "manifest.txt").Split('\n'))
            {
                string[] a = line.TrimEnd('\r').Split(new[] { ' ' }, 4);
                if (a.Length == 2 && a[0] == "vault") vault = a[1];
                long size;
                if (a.Length == 4 && (a[0] == "f" || a[0] == "d") && long.TryParse(a[2], out size) && !a[3].Contains("..") && !Path.IsPathRooted(a[3]))
                    entries.Add(new Entry { mustMatch = a[0] == "f", sha = a[1], size = size, path = a[3] });
            }
        }
        catch (Exception ex) { return "No se pudo leer la lista de archivos del servidor: " + ex.Message; }
        if (entries.Count == 0) return "El servidor no publicó la lista de archivos del juego.";
        Program.Log("vault " + vault + ": " + entries.Count + " files");

        // What was hashed before: path -> size|last write|sha. A file whose size and time did not change is not hashed again.
        string cacheFile = Path.Combine(gameDir, ".grp-files");
        var cache = new Dictionary<string, string>();
        try { if (File.Exists(cacheFile)) foreach (string l in File.ReadAllLines(cacheFile)) { int t = l.IndexOf('\t'); if (t > 0) cache[l.Substring(0, t)] = l.Substring(t + 1); } } catch { }
        Func<Entry, string, bool> same = (e, file) =>
        {
            var fi = new FileInfo(file);
            if (!fi.Exists || fi.Length != e.size) return false;
            string key = fi.Length + "|" + fi.LastWriteTimeUtc.Ticks, c;
            if (cache.TryGetValue(e.path, out c) && c == key + "|" + e.sha) return true;
            if (Sha256(file) != e.sha) return false;
            cache[e.path] = key + "|" + e.sha;
            return true;
        };

        var need = new List<Entry>();
        try
        {
            Directory.CreateDirectory(gameDir);
            for (int i = 0; i < entries.Count; i++)
            {
                Entry e = entries[i];
                string file = Path.Combine(gameDir, e.path.Replace('/', Path.DirectorySeparatorChar));
                if (i % 20 == 0) progress("Comprobando archivos del juego...", (double)i / entries.Count);
                if (!e.mustMatch && File.Exists(file)) continue;
                if (!same(e, file)) need.Add(e);
            }
            long total = need.Sum(e => e.size), done = 0;
            Program.Log(need.Count + " files to get, " + total / (1 << 20) + " MB");
            string phoenix = Config.PhoenixGameDir;
            bool usePhoenix = Directory.Exists(phoenix) && !string.Equals(Path.GetFullPath(phoenix).TrimEnd('\\', '/'), Path.GetFullPath(gameDir).TrimEnd('\\', '/'), StringComparison.OrdinalIgnoreCase);
            int copied = 0, fetched = 0;
            foreach (Entry e in need)
            {
                string rel = e.path.Replace('/', Path.DirectorySeparatorChar), file = Path.Combine(gameDir, rel);
                Action<long> step = n =>
                {
                    done += n;
                    progress("Descargando " + Path.GetFileName(rel) + "  (" + done / (1 << 20) + " de " + total / (1 << 20) + " MB)", total == 0 ? 1 : (double)done / total);
                };
                string other = Path.Combine(phoenix, rel);
                bool got = false;
                if (usePhoenix && File.Exists(other) && new FileInfo(other).Length == e.size)
                {
                    progress("Copiando " + Path.GetFileName(rel) + " de la instalación de Phoenix...", total == 0 ? 1 : (double)done / total);
                    if (Sha256(other) == e.sha)
                    {
                        Directory.CreateDirectory(Path.GetDirectoryName(file));
                        File.Copy(other, file, true);
                        done += e.size; copied++; got = true;
                    }
                }
                if (!got) { Download(baseUrl + "files/" + Uri.EscapeUriString(e.path), file, e.size, e.sha, step); fetched++; }
                var fi = new FileInfo(file);
                cache[e.path] = fi.Length + "|" + fi.LastWriteTimeUtc.Ticks + "|" + e.sha;
            }
            Program.Log("game folder ready: " + copied + " copied from Phoenix, " + fetched + " downloaded");
            File.WriteAllLines(cacheFile, cache.Select(kv => kv.Key + "\t" + kv.Value).ToArray());
        }
        catch (Exception ex)
        {
            try { File.WriteAllLines(cacheFile, cache.Select(kv => kv.Key + "\t" + kv.Value).ToArray()); } catch { }
            Program.Log("sync failed: " + ex.Message);
            return ex is UnauthorizedAccessException ? "No hay permiso para escribir en la carpeta del juego:\n" + gameDir : ex.Message;
        }
        return null;
    }
}

// The pipe the game's own launcher protocol runs on. Messages are a sequence of [u32 byte length][UTF-16LE text];
// the game sends ["v2"][command]. Answers are binary: a status byte 01 followed by the value.
static class GamePipe
{
    [DllImport("kernel32.dll", SetLastError = true, CharSet = CharSet.Unicode)]
    static extern IntPtr CreateNamedPipeW(string name, uint openMode, uint pipeMode, uint maxInstances, uint outSize, uint inSize, uint timeout, IntPtr sa);
    [DllImport("kernel32.dll", SetLastError = true)] static extern bool ConnectNamedPipe(IntPtr h, IntPtr ov);
    [DllImport("kernel32.dll", SetLastError = true)] static extern bool DisconnectNamedPipe(IntPtr h);
    [DllImport("kernel32.dll", SetLastError = true)] static extern bool ReadFile(IntPtr h, byte[] buf, uint len, out uint read, IntPtr ov);
    [DllImport("kernel32.dll", SetLastError = true)] static extern bool WriteFile(IntPtr h, byte[] buf, uint len, out uint written, IntPtr ov);
    [DllImport("kernel32.dll", SetLastError = true)] static extern bool CloseHandle(IntPtr h);
    const uint PIPE_ACCESS_DUPLEX = 3, PIPE_TYPE_MESSAGE = 4, PIPE_READMODE_MESSAGE = 2;

    static string user = "";
    static bool started;

    public static void Start(string gameUser)
    {
        user = gameUser;
        if (started) return;
        started = true;
        // The game fires several requests back to back, each on a new connection: several instances listen.
        for (int i = 0; i < 4; i++) new Thread(Serve) { IsBackground = true }.Start();
    }

    static void Serve()
    {
        while (true)
        {
            IntPtr h = CreateNamedPipeW(@"\\.\pipe\Ghost_Recon_Phantoms", PIPE_ACCESS_DUPLEX, PIPE_TYPE_MESSAGE | PIPE_READMODE_MESSAGE, 255, 4096, 4096, 0, IntPtr.Zero);
            if (h == new IntPtr(-1)) { Program.Log("pipe: create failed " + Marshal.GetLastWin32Error()); Thread.Sleep(1000); continue; }
            if (!ConnectNamedPipe(h, IntPtr.Zero) && Marshal.GetLastWin32Error() != 535) { CloseHandle(h); continue; }
            var buf = new byte[4096];
            uint r, w;
            while (ReadFile(h, buf, (uint)buf.Length, out r, IntPtr.Zero) && r > 0)
            {
                var parts = Unframe(buf.Take((int)r).ToArray());
                Program.Log("pipe: " + string.Join(" ", parts));
                byte[] reply = Reply(parts);
                if (reply != null) WriteFile(h, reply, (uint)reply.Length, out w, IntPtr.Zero);
            }
            DisconnectNamedPipe(h);
            CloseHandle(h);
        }
    }

    static List<string> Unframe(byte[] m)
    {
        var parts = new List<string>();
        int o = 0;
        while (o + 4 <= m.Length)
        {
            int len = BitConverter.ToInt32(m, o); o += 4;
            if (len < 0 || o + len > m.Length) break;
            parts.Add(Encoding.Unicode.GetString(m, o, len)); o += len;
        }
        return parts;
    }

    static byte[] Frame(params string[] parts)
    {
        var ms = new MemoryStream();
        foreach (var p in parts)
        {
            var b = Encoding.Unicode.GetBytes(p);
            ms.Write(BitConverter.GetBytes(b.Length), 0, 4);
            ms.Write(b, 0, b.Length);
        }
        return ms.ToArray();
    }

    static byte[] Cat(params byte[][] parts) { return parts.SelectMany(p => p).ToArray(); }

    static byte[] Reply(List<string> request)
    {
        if (request.Count < 2) return null;
        byte[] ok = { 1 };
        switch (request[1])
        {
            case "epochnow": return Cat(ok, BitConverter.GetBytes((long)(DateTime.UtcNow - new DateTime(1970, 1, 1)).TotalSeconds));
            case "validate": return ok;
            case "notifyclientstart": return Frame("ok");
            case "showeula": return new byte[] { 1, 0 };
            case "getdistributionsource": return Cat(ok, Frame(""));
            case "getlogininfo": return Cat(ok, Frame(user, "grp"));   // the server checks the token inside the user string
            case "getproxy": return new byte[] { 1, 0, 0, 0, 0, 0 };
            default: return ok;
        }
    }
}

static class Program
{
    static readonly object logLock = new object();
    public static void Log(string s)
    {
        try
        {
            Directory.CreateDirectory(Config.DataDir);
            lock (logLock) File.AppendAllText(Path.Combine(Config.DataDir, "launcher.log"), DateTime.Now.ToString("yyyy-MM-dd HH:mm:ss.fff") + " " + s + Environment.NewLine);
        }
        catch { }
    }

    // Signs in and starts the game. Returns null when the game ran and exited, or the message for the player.
    public static string Play(string name, string password, string gameDir, bool sync, Action<string, double> progress)
    {
        Action<string> status = s => progress(s, -1);
        string gameExe = Path.Combine(gameDir, Config.GameExe);
        status("Iniciando sesión...");
        string answer = Account.Login(name, password);
        string[] a = answer.Split(' ');
        if (a.Length < 3 || a[0] != "OK") { Log("login refused: " + (answer.StartsWith("OK") ? "bad answer" : answer)); return Account.Explain(answer); }
        Log("signed in as " + a[1]);
        if (sync && a.Length >= 4)
        {
            string err = Updater.SyncGame(a[3], gameDir, progress);
            if (err != null) return err;
        }
        if (!File.Exists(gameExe)) return "No se encontró el juego en:\n" + gameExe;
        GamePipe.Start(a[1] + "." + a[2]);
        Thread.Sleep(500);
        status("Abriendo el juego...");
        var psi = new ProcessStartInfo(gameExe, "/ForceDefaultSandbox=true /Sandbox=" + Config.Server + " /Port=" + Config.Port + " /OnlineAccessKey=" + Config.AccessKey + " /language=" + Config.Language)
        {
            UseShellExecute = false,
            WorkingDirectory = Path.GetDirectoryName(gameExe)
        };
        Process game;
        try { game = Process.Start(psi); }
        catch (Exception ex) { Log("game start failed: " + ex.Message); return "No se pudo abrir el juego: " + ex.Message; }
        Log("game started, pid " + game.Id);
        status("Jugando. Esta ventana vuelve cuando cierres el juego.");
        game.WaitForExit();
        Log("game exited, code " + game.ExitCode);
        return null;
    }

    static string Arg(string[] args, string key)
    {
        int i = Array.IndexOf(args, key);
        return i >= 0 && i + 1 < args.Length ? args[i + 1] : null;
    }

    [STAThread]
    static int Main(string[] args)
    {
        Config.Load();
        Log("launcher " + Config.Version + " started, server " + Config.Server);
        if (Updater.SelfUpdate(args)) return 0;
        if (Arg(args, "--user") != null)
        {
            string name = Arg(args, "--user"), pw = Arg(args, "--password") ?? "", invite = Arg(args, "--invite");
            if (invite != null)
            {
                string r = Account.Register(invite, name, pw);
                Console.WriteLine("register: " + r);
                if (r != "OK") return 2;
            }
            string last = "";
            string err = Play(name, pw, Arg(args, "--gamedir") ?? Config.DefaultGameDir, Array.IndexOf(args, "--nosync") < 0, (s, f) => { if (s != last) Log("status: " + s); last = s; });
            if (err != null) { Console.WriteLine(err); return 1; }
            return 0;
        }
        Application.EnableVisualStyles();
        Application.Run(new LauncherForm());
        return 0;
    }
}

class LauncherForm : Form
{
    readonly TextBox name = new TextBox(), password = new TextBox(), invite = new TextBox(), game = new TextBox();
    readonly Button play = new Button(), browse = new Button();
    readonly CheckBox create = new CheckBox();
    readonly Label status = new Label(), inviteLabel = new Label();
    readonly ProgressBar bar = new ProgressBar();
    readonly string settingsFile = Path.Combine(Config.DataDir, "settings.txt");

    Label Caption(string text, int y)
    {
        var l = new Label { Text = text, Left = 24, Top = y, Width = 372, ForeColor = Color.FromArgb(150, 170, 180) };
        Controls.Add(l);
        return l;
    }

    void Field(TextBox t, int y, int width = 372)
    {
        t.Left = 24; t.Top = y; t.Width = width;
        t.BackColor = Color.FromArgb(30, 38, 44); t.ForeColor = Color.White; t.BorderStyle = BorderStyle.FixedSingle;
        Controls.Add(t);
    }

    public LauncherForm()
    {
        Text = "GRP Launcher";
        ClientSize = new Size(420, 424);
        FormBorderStyle = FormBorderStyle.FixedSingle; MaximizeBox = false; StartPosition = FormStartPosition.CenterScreen;
        BackColor = Color.FromArgb(18, 24, 28); ForeColor = Color.White; Font = new Font("Segoe UI", 10f);

        Controls.Add(new Label { Text = "GHOST RECON PHANTOMS", Left = 24, Top = 18, Width = 372, Height = 28, Font = new Font("Segoe UI", 14f, FontStyle.Bold), ForeColor = Color.FromArgb(120, 220, 230) });
        Caption("Nickname", 58); Field(name, 80);
        Caption("Contraseña", 114); Field(password, 136); password.UseSystemPasswordChar = true;

        create.Text = "Crear cuenta nueva"; create.Left = 24; create.Top = 172; create.Width = 372; create.ForeColor = Color.FromArgb(200, 210, 215);
        create.CheckedChanged += (s, e) => { invite.Visible = inviteLabel.Visible = create.Checked; play.Text = create.Checked ? "CREAR CUENTA Y JUGAR" : "JUGAR"; };
        Controls.Add(create);
        inviteLabel.Text = "Código de invitación"; inviteLabel.Left = 24; inviteLabel.Top = 200; inviteLabel.Width = 372; inviteLabel.ForeColor = Color.FromArgb(150, 170, 180); inviteLabel.Visible = false;
        Controls.Add(inviteLabel);
        Field(invite, 222); invite.Visible = false;

        Caption("Carpeta del juego (se descarga ahí si falta)", 258); Field(game, 280, 282);
        browse.Text = "Buscar"; browse.Left = 314; browse.Top = 279; browse.Width = 82; browse.Height = 27; browse.FlatStyle = FlatStyle.Flat; browse.ForeColor = Color.White;
        browse.Click += (s, e) =>
        {
            using (var d = new FolderBrowserDialog { Description = "Carpeta donde está o se instalará el juego (unos 4 GB)", SelectedPath = Directory.Exists(game.Text) ? game.Text : "" })
                if (d.ShowDialog(this) == DialogResult.OK) game.Text = d.SelectedPath;
        };
        Controls.Add(browse);

        play.Text = "JUGAR"; play.Left = 24; play.Top = 322; play.Width = 372; play.Height = 40; play.FlatStyle = FlatStyle.Flat;
        play.BackColor = Color.FromArgb(70, 140, 110); play.ForeColor = Color.White; play.Font = new Font("Segoe UI", 11f, FontStyle.Bold);
        play.Click += (s, e) => Go();
        Controls.Add(play);
        AcceptButton = play;

        bar.Left = 24; bar.Top = 368; bar.Width = 372; bar.Height = 8; bar.Maximum = 1000; bar.Visible = false;
        Controls.Add(bar);
        status.Left = 24; status.Top = 382; status.Width = 372; status.Height = 36; status.ForeColor = Color.FromArgb(230, 190, 110); status.Font = new Font("Segoe UI", 8.5f);
        Controls.Add(status);

        var saved = Config.ReadPairs(settingsFile);
        string v;
        name.Text = saved.TryGetValue("name", out v) ? v : "";
        game.Text = saved.TryGetValue("gamedir", out v) && v != "" ? v : Config.DefaultGameDir;
        Shown += (s, e) => { if (name.Text == "") name.Focus(); else password.Focus(); };
    }

    void SetStatus(string s) { if (IsHandleCreated) BeginInvoke((Action)(() => status.Text = s)); }

    void Go()
    {
        string n = name.Text.Trim(), p = password.Text, inv = invite.Text.Trim(), g = game.Text.Trim();
        bool register = create.Checked;
        if (n == "" || p == "") { status.Text = "Escribe tu nickname y tu contraseña."; return; }
        if (register && p.Length < 6) { status.Text = "La contraseña debe tener al menos 6 caracteres."; return; }
        if (register && inv == "") { status.Text = "Escribe el código de invitación."; return; }
        try { Directory.CreateDirectory(Config.DataDir); File.WriteAllLines(settingsFile, new[] { "name=" + n, "gamedir=" + g }); } catch { }
        play.Enabled = false;
        new Thread(() =>
        {
            string err = null;
            if (register)
            {
                SetStatus("Creando la cuenta...");
                string r = Account.Register(inv, n, p);
                Program.Log("register " + n + ": " + r);
                if (r != "OK") err = Account.Explain(r);
            }
            if (err == null)
            {
                if (register) BeginInvoke((Action)(() => create.Checked = false));
                BeginInvoke((Action)(() => WindowState = FormWindowState.Normal));
                err = Program.Play(n, p, g, true, (s, f) =>
                {
                    BeginInvoke((Action)(() =>
                    {
                        status.Text = s;
                        bar.Visible = f >= 0;
                        if (f >= 0) bar.Value = (int)(Math.Max(0, Math.Min(1, f)) * 1000);
                        if (s.StartsWith("Jugando")) WindowState = FormWindowState.Minimized;
                    }));
                });
            }
            BeginInvoke((Action)(() =>
            {
                WindowState = FormWindowState.Normal;
                bar.Visible = false;
                status.Text = err ?? "";
                play.Enabled = true;
                password.Text = "";
            }));
        }) { IsBackground = true }.Start();
    }
}
