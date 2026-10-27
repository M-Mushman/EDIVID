"""Temps de réverbération apparent d'une voix (RT60 estimé à l'aveugle) : enveloppes d'énergie par bande (pas de
10 ms), pentes des décroissances libres d'au moins 120 ms ; les décroissances les plus raides (90e centile) donnent
la pente de la pièce, RT60 = 60 dB / pente. Sert à classer des traitements, pas à mesurer une salle.
Usage : python rt60.py <wav>…
"""
import subprocess, sys
import numpy as np
SR = 16000; N = 512; PAS = 160
BANDES = [(250, 500), (500, 1000), (1000, 2000), (2000, 4000), (4000, 7000)]
def lire(f):
    b = subprocess.run(["ffmpeg","-v","error","-i",f,"-ac","1","-ar",str(SR),"-f","f32le","-"],capture_output=True,check=True).stdout
    return np.frombuffer(b, np.float32)
def enveloppes(x):
    fen = np.hanning(N); nf = (len(x)-N)//PAS+1
    idx = np.arange(N)[None,:]+np.arange(nf)[:,None]*PAS
    P = np.abs(np.fft.rfft(x[idx]*fen,axis=1))**2
    f = np.fft.rfftfreq(N,1/SR)
    return [10*np.log10(P[:,(f>=lo)&(f<hi)].mean(axis=1)+1e-12) for lo,hi in BANDES]
def pentes(env, mini=12):
    """Pentes (dB/s) des segments strictement décroissants d'au moins `mini` trames (120 ms), partant d'un niveau fort."""
    seuil = np.percentile(env, 70)
    res = []; i = 0
    while i < len(env)-1:
        if env[i] >= seuil:
            j = i
            while j+1 < len(env) and env[j+1] < env[j] - 0.05: j += 1
            if j-i >= mini and env[i]-env[j] >= 10:
                t = np.arange(i, j+1)*PAS/SR
                res.append(-np.polyfit(t, env[i:j+1], 1)[0])
            i = max(j, i+1)
        else:
            i += 1
    return res
for f in sys.argv[1:]:
    x = lire(f); rts = []
    for env in enveloppes(x):
        p = pentes(env)
        if len(p) >= 3: rts.append(60/np.percentile(p, 90))
    print(f"{f.split('/')[-1]:<44} RT60 apparent {np.median(rts)*1000:5.0f} ms  (bandes : {' '.join(f'{r*1000:.0f}' for r in rts)})")
