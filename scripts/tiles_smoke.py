#!/usr/bin/env python3
"""Real-input tile authoring, graph placement, generation, persistence and curb/door traversal."""
import json
import math
import os
from pathlib import Path
import select
import shutil
import subprocess
import time
from PIL import Image
from creator_smoke import Session, float32_model
from visual_support import ROOT, stop, wait_for


def logical(library):
    library=dict(library)
    for key in ("bodies","props","characters","tiles","worlds"):
        library[key]=sorted(library[key],key=lambda asset:asset["id"])
    return float32_model(library)

def main():
    output=ROOT/'artifacts'/f'tiles-{time.strftime("%Y%m%d-%H%M%S")}-{os.getpid()}'
    output.mkdir(parents=True)
    data=output/'data'
    shutil.copytree(ROOT/'test_workspace',data)
    report=dict(status='failed',renderer='Mesa software Vulkan',actions=[],captures=[])
    env=os.environ.copy()
    drivers=[p for d in env.get('XDG_DATA_DIRS','/usr/share').split(':') for p in Path(d).glob('vulkan/icd.d/lvp*.json')]
    env.update(XDG_CACHE_HOME=str(ROOT/'artifacts/cache'),VK_DRIVER_FILES=str(drivers[-1]),VK_ICD_FILENAMES=str(drivers[-1]),WINIT_UNIX_BACKEND='x11',WINIT_X11_SCALE_FACTOR='1',WGPU_BACKEND='vulkan',RUST_LOG='info,wgpu_core=warn,wgpu_hal=warn')
    env.pop('WAYLAND_DISPLAY',None)
    xvfb=s=game=None
    try:
        rd,wr=os.pipe()
        with (output/'xvfb.log').open('w') as log:
            xvfb=subprocess.Popen(['Xvfb','-displayfd',str(wr),'-screen','0','1600x900x24','-nolisten','tcp','-ac'],pass_fds=(wr,),env=env,stdout=log,stderr=subprocess.STDOUT)
        os.close(wr)
        assert select.select([rd],[],[],15)[0]
        with os.fdopen(rd) as display_pipe:
            env['DISPLAY']=':'+display_pipe.readline().strip()
        s=Session(env,output,data,report,workspace='tiles',prefix='tiles')
        def action(name): return s.action('World('+name+')')
        def capture(name): s.capture(name,neutral=True)
        def tile_named(name):
            for _ in range(len(s.state()['library']['tiles'])):
                st=s.state()
                if st['library']['tiles'][st['tile']]['name']==name:return
                action('Tile(1)')
            raise AssertionError(name)
        for name in ['Grass','House plot','Road straight','Road corner','Road T','Road crossing','Front door','Roof slope','Tree trunk']:
            tile_named(name)
            if name in ['Road straight','Road corner','Front door']:
                capture(name.lower().replace(' ','-'))
        # Work on a new source asset through visible controls, retaining the shared world's valid sockets.
        action('NewTile')
        action('Brush(8)')
        action('Paint(false)')
        st=s.state();assert st['library']['tiles'][st['tile']]['voxels']
        action('Tool(1)');action('AddSocket');action('SocketRequired');action('Cursor(0, 1)');action('SocketTurn')
        def rename_field(target,text):
            s.action('Rename('+target+')');s.xdo('key','ctrl+a');s.wait(lambda v:v.get('naming')=='','clear field')
            s.xdo('type','--delay',40,text);s.xdo('key','Return');s.wait(lambda v:v.get('naming') is None,'commit field')
        rename_field('SocketType','custom-path');rename_field('SocketProfile','custom-width')
        action('Tool(3)');rename_field('RuleLeft','custom-path');rename_field('RuleRight','custom-path');action('ToggleRule')
        assert ['custom-path','custom-path'] in s.state()['library']['socket_rules']['pairs']

        action('Tool(2)');action('Collider')
        st=s.state();assert st['library']['tiles'][st['tile']]['colliders']
        s.action('Undo');s.action('Redo')
        s.action('Frame');capture('tile-authoring')
        s.action('Navigate(Worlds)')
        action('Validate');capture('neighbourhood')
        original_id=s.state()['library']['worlds'][0]['id']
        action('NewWorld')
        tile_named('Road straight');action('Place')
        # Source socket selection and rotation must be real controls, not injected document state.
        tile_named('Road corner');action('Rotate');action('Rotate');action('Place')
        st=s.state();w=st['library']['worlds'][st['world']]
        assert len(w['instances'])==2 and len(w['connections'])==1
        s.action('Frame');capture('manual-road-corner')
        action('Remove');s.action('Undo')
        assert len(s.state()['library']['worlds'][st['world']]['instances'])==2
        action('Seed');action('Generate');action('Validate')
        s.action('Save');expected=s.state()['library'];capture('generated-world')
        s.action('Reload');assert logical(s.state()['library'])==logical(expected)
        stop(s.process);s=None
        s=Session(env,output,data,report,workspace='worlds',prefix='reopened')
        assert logical(s.state()['library'])==logical(expected)
        stop(s.process);s=None
        state_path=output/'play-state.json'
        command=[str(ROOT/'target/debug/game'),'--mode','game','--workspace',str(data),'--world',str(original_id),'--state-file',str(state_path),'--output-dir',str(output/'play'),'--capture',str(output/'play-ready.png'),'--capture-frame','60']
        with (output/'play.log').open('w') as log:game=subprocess.Popen(command,env=env,stdout=log,stderr=subprocess.STDOUT)
        def state():
            try:return json.loads(state_path.read_text())
            except (OSError,ValueError):return {}
        def wait(pred,label):
            wait_for(lambda:pred(state()),label,game,timeout=120)
            return state()
        wait(lambda v:v.get('world') and v.get('collision_boxes',0)>0,'world startup')
        wait_for(lambda:'capture_saved' in (output/'play.log').read_text(),'renderer warmup',game,timeout=120)
        def xdo(*args):return subprocess.check_output(['xdotool',*map(str,args)],env=env,text=True,timeout=10).strip()
        window=xdo('search','--sync','--onlyvisible','--pid',game.pid).splitlines()[0]
        xdo('windowfocus','--sync',window)
        xdo('mousemove','--window',window,640,360);xdo('click',1)
        wait(lambda v:v.get('captured'),'capture play controls')
        start=state()['position'];yaw=state()['camera_yaw']
        forward=(-math.sin(yaw),-math.cos(yaw))
        def distance(v):return (v['position'][0]-start[0])*forward[0]+(v['position'][2]-start[2])*forward[1]
        xdo('keydown','w')
        curb=wait(lambda v:distance(v)>1. and v['position'][1]>.18,'step from road onto sidewalk')
        door=wait(lambda v:distance(v)>10.7,'walk along path and through front door')
        xdo('keyup','w')
        wait(lambda v:v['speed']<.01,'stop inside house')
        assert abs(door['position'][1]-.2)<.03
        xdo('key','F12');capture=output/'play/screenshot-001.png'
        wait_for(lambda:capture.exists(),'house capture',game)
        time.sleep(1)
        with Image.open(capture) as im:assert im.size==(1280,720)
        shutil.copy(capture,output/'inside-house.png')
        xdo('key','space');wait(lambda v:not v['grounded'],'jump inside house');wait(lambda v:v['grounded'],'land on house floor')
        # Continue into the back wall and verify collision stops movement.
        xdo('keydown','w');time.sleep(4);xdo('keyup','w');wait(lambda v:v['speed']<.01,'wall stop')
        wall=state();assert 14<distance(wall)<16.4,(distance(wall),wall['position'])
        for name in ['tiles.log','reopened.log','play.log']:
            log=(output/name).read_text();assert ' ERROR ' not in log and 'panicked at' not in log,name
        report.update(status='passed',game_command=command,gameplay={'curb':curb['position'],'door':door['position'],'wall':wall['position']})
        print('PASS: voxel/socket/collision editing, rotated placement, seeded generation, restart, curb, doorway and wall collision.',flush=True)
    except Exception as error:
        report['error']=repr(error)
        raise
    finally:
        if s:stop(s.process)
        stop(game);stop(xvfb)
        (output/'report.json').write_text(json.dumps(report,indent=2)+'\n')
        print('Report:',output/'report.json',flush=True)

if __name__=='__main__':main()
