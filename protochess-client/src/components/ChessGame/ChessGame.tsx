import React, {Component} from "react";
import {GameState,Piece} from "protochess-shared";
import {fabric} from "fabric";
import Board from "./Board";
import PieceHandler from "./PieceHandler";
import {ColorConstants} from "./ColorConstants";

interface IProps {
    width:number,
    height:number,
    gameState:GameState,
    requestMove:Function,
    inverted:boolean | null,
    playerNum:number | null
}
export default class ChessGame extends Component<IProps> {
    private tileMap:Map<string,fabric.Object> = new Map();
    private pieceMap:Map<string,fabric.Object> = new Map();
    private tileWidth:number = 0;
    private tileHeight:number = 0;
    private canvas:fabric.Canvas | null = null;
    componentDidUpdate(prevProps: Readonly<IProps>, prevState: Readonly<{}>, snapshot?: any): void {
        if (this.props.playerNum !== null
            && this.props.inverted !== null
            && this.props.inverted !== prevProps.inverted){
            PieceHandler.deleteAllPieces();
            Board.deleteAll();
            //Maps X#Y# to a fabric rect
            this.tileMap = Board.loadBoard(this.props.inverted, this.props.gameState);

            //Maps pieceID to fabric group
            this.pieceMap = PieceHandler.loadPieces(this.props.playerNum,this.props.inverted, this, this.props.gameState);
        }
    }

    componentDidMount(): void {
        let canvas = new fabric.Canvas('myCanvas');
        this.canvas = canvas;
        canvas.preserveObjectStacking = true;
        canvas.setHeight(this.props.height);
        canvas.setWidth(this.props.width);
        canvas.selection = false;

        canvas.on({
            'object:moving': function(e) {
                e!.target!.opacity = 0.6;
            },
            'object:modified': function(e) {
                e!.target!.opacity = 1;
            }
        });

        this.tileWidth = canvas.getWidth()/this.props.gameState.board.width;
        this.tileHeight = canvas.getHeight()/this.props.gameState.board.height;
        Board.init(canvas,this.tileWidth,this.tileHeight);
        PieceHandler.init(canvas,this.tileWidth,this.tileHeight);

    }

    updatePiece(piece:Piece) {
        let group = PieceHandler.pieceMap.get(piece.id);
        if (group){
            let oldXY = Board.fabricPosToXY(this.props.inverted!,group!.left!,group!.top!);
            Board.updateBoardHighlight(this.props.inverted!,oldXY['x'],oldXY['y'],ColorConstants.LAST_MOVE_HIGHLIGHT_COLOR);
            console.log(oldXY['y']);

        }
        PieceHandler.updatePiece(this.props.gameState,piece);
        Board.updateBoardHighlight(this.props.inverted!,piece.location.x,piece.location.y,ColorConstants.LAST_MOVE_HIGHLIGHT_COLOR);
        console.log(piece.location.y);
    }

    deletePiece(piece:Piece){
        PieceHandler.deletePiece(piece);
    }

    lockAllPieces(playerNum:number){
        PieceHandler.lockAllPieces();
    }

    unlockPieces(playerNum:number){
        PieceHandler.unlockPieces(playerNum);
    }

    setTileHighlight(x:number,y:number,color:string){
        Board.setHighlightColor(this.props.inverted!,x,y,color)

    }

    displayWinner(playerNum:number){
        if (this.canvas){
            var text = new fabric.Text(playerNum +' wins!', {
                left: 0,
                top: 0,
                fill: 'black'
            });
            this.canvas.add(text);
            text.center();
            this.canvas.renderAll();
        }
    }

    render() {
        return (
            <div id="chessGame">
                <canvas id="myCanvas"/>
            </div>
        );
    }

    requestMove(move: {x: number; y: number; id: any}) {
        this.props.requestMove(move);
    }

}

